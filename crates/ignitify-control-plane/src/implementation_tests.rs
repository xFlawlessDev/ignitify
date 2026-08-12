use std::collections::HashSet;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use age::secrecy::ExposeSecret;
use ignitify_db::{
    Database, DatabaseConfig, DeploymentActor, NewServiceVariable, ProjectActor, ServiceActor,
    ServiceVariableRecord, UserRole as DatabaseUserRole,
};
use ignitify_domain::{
    DnsRecord, DnsRecordType, DnsVerificationStatus, DomainName, DomainStatus, ProjectInput,
    ServiceInput, ServiceVariableInput,
};

use super::{
    AgeCipher, ImageRuntime, Ingress, ProjectEnvironmentVariableInput, ServiceControl,
    reconcile_once,
};
use crate::{DnsVerificationResult, IngressRoute, RuntimeLog};

#[test]
fn deployment_logs_redact_snapshot_values() {
    let logs = crate::deployment_values::redact_logs(
        vec![ignitify_db::NewDeploymentLog {
            stream: "stdout".to_owned(),
            line: "TOKEN=plain-secret".to_owned(),
        }],
        &[zeroize::Zeroizing::new("plain-secret".to_owned())],
    );

    assert_eq!(logs[0].line, "[REDACTED]");
}

#[test]
fn deployment_logs_redact_when_any_variable_is_present() {
    let logs = crate::deployment_values::redact_logs(
        vec![ignitify_db::NewDeploymentLog {
            stream: "stdout".to_owned(),
            line: "safe output".to_owned(),
        }],
        &[zeroize::Zeroizing::new("plain-secret".to_owned())],
    );

    assert_eq!(logs[0].line, "[REDACTED]");
}

#[test]
fn deployment_logs_redact_multiline_snapshot_values() {
    let logs = crate::deployment_values::redact_logs(
        vec![
            ignitify_db::NewDeploymentLog {
                stream: "stdout".to_owned(),
                line: "TOKEN=first".to_owned(),
            },
            ignitify_db::NewDeploymentLog {
                stream: "stdout".to_owned(),
                line: "second".to_owned(),
            },
        ],
        &[zeroize::Zeroizing::new("first\nsecond".to_owned())],
    );

    assert_eq!(
        logs.into_iter().map(|log| log.line).collect::<Vec<_>>(),
        ["[REDACTED]", "[REDACTED]"]
    );
}

#[test]
fn ciphertext_excludes_plaintext() {
    let identity = age::x25519::Identity::generate();
    let identity = identity.to_string();
    let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
    let ciphertext = cipher.encrypt(b"not-in-ciphertext").unwrap();

    assert!(!ciphertext.contains("not-in-ciphertext"));
    assert_eq!(
        cipher.decrypt(&ciphertext).unwrap().as_slice(),
        b"not-in-ciphertext"
    );
}

#[tokio::test]
async fn service_update_preserves_existing_secret_ciphertext() {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let identity = age::x25519::Identity::generate().to_string();
    let control = ServiceControl::new(
        database.services(),
        database.projects(),
        identity.expose_secret(),
    )
    .unwrap();
    let input = ServiceInput::image(
        "web",
        "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some(80),
        None,
        vec![ServiceVariableInput {
            key: "API_TOKEN".to_owned(),
            value: String::new(),
            is_secret: true,
        }],
    )
    .unwrap();

    let (_, variables) = control
        .encrypt_variables_preserving_secrets(
            input,
            &[ServiceVariableRecord {
                key: "API_TOKEN".to_owned(),
                is_secret: true,
                ciphertext: "existing-ciphertext".to_owned(),
            }],
            &HashSet::from(["API_TOKEN".to_owned()]),
        )
        .unwrap();

    assert_eq!(variables[0].ciphertext, "existing-ciphertext");
}

struct FakeIngress;

impl Ingress for FakeIngress {
    fn route(
        &self,
        _service_id: &ignitify_domain::ServiceId,
        _domain_id: &ignitify_domain::DomainId,
        _hostname: &ignitify_domain::DomainName,
        _port: u32,
    ) -> super::Result<IngressRoute> {
        Ok(IngressRoute {
            labels: std::collections::BTreeMap::new(),
            network: "none".to_owned(),
        })
    }
}

struct SyncingIngress(Arc<AtomicBool>);

impl Ingress for SyncingIngress {
    fn route(
        &self,
        _service_id: &ignitify_domain::ServiceId,
        _domain_id: &ignitify_domain::DomainId,
        _hostname: &ignitify_domain::DomainName,
        _port: u32,
    ) -> super::Result<IngressRoute> {
        Ok(IngressRoute {
            labels: std::collections::BTreeMap::new(),
            network: "none".to_owned(),
        })
    }

    async fn reconcile(&self) -> super::Result<()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
}

struct ValidDnsVerifier;

impl crate::DnsVerifier for ValidDnsVerifier {
    async fn verify(&self, _domain: &ignitify_db::DomainRecord) -> DnsVerificationResult {
        DnsVerificationResult {
            status: DnsVerificationStatus::Valid,
            error: None,
        }
    }
}

struct FakeRuntime {
    calls: Arc<Mutex<Vec<String>>>,
    routed_local_images: Arc<Mutex<Vec<Option<String>>>>,
    logs: Arc<Mutex<Vec<Vec<RuntimeLog>>>>,
    routes_fail: bool,
}

#[tokio::test]
async fn worker_syncs_ingress_before_claiming_deployments() {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let identity = age::x25519::Identity::generate().to_string();
    let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
    let synchronized = Arc::new(AtomicBool::new(false));
    let ingress = SyncingIngress(synchronized.clone());
    let runtime = FakeRuntime {
        calls: Arc::new(Mutex::new(vec![])),
        routed_local_images: Arc::new(Mutex::new(vec![])),
        logs: Arc::new(Mutex::new(vec![])),
        routes_fail: false,
    };
    let (publisher, _) = tokio::sync::broadcast::channel(16);

    reconcile_once(
        &database.deployments(),
        &database.domains(),
        &cipher,
        &runtime,
        &ingress,
        &super::StreamPublisher::new(publisher),
    )
    .await
    .unwrap();

    assert!(synchronized.load(Ordering::Acquire));
}

#[tokio::test]
async fn worker_completes_requested_dns_verification() {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let actor_id = database
        .users()
        .create("owner", "hash", DatabaseUserRole::User)
        .await
        .unwrap()
        .id;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    let service = database
        .services()
        .create(
            ServiceActor {
                id: &actor_id,
                is_admin: false,
            },
            project.id.as_str(),
            ServiceInput::image(
                "web",
                "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some(8080),
                None,
                vec![],
            )
            .unwrap()
            .configuration,
            Vec::<NewServiceVariable>::new(),
            None,
        )
        .await
        .unwrap();
    let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
        panic!("service must exist");
    };
    let actor = ignitify_db::DomainActor {
        id: &actor_id,
        is_admin: false,
    };
    let domain = database
        .domains()
        .create(
            actor,
            service.id.as_str(),
            DomainName::new("app.example.com").unwrap(),
            DnsRecord::new(DnsRecordType::A, "203.0.113.10").unwrap(),
        )
        .await
        .unwrap();
    let ignitify_db::DomainMutationOutcome::Created(domain) = domain else {
        panic!("domain must exist");
    };
    database
        .domains()
        .request_dns_verification(actor, domain.id.as_str())
        .await
        .unwrap();

    let identity = age::x25519::Identity::generate().to_string();
    let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
    let runtime = FakeRuntime {
        calls: Arc::new(Mutex::new(vec![])),
        routed_local_images: Arc::new(Mutex::new(vec![])),
        logs: Arc::new(Mutex::new(vec![])),
        routes_fail: false,
    };
    let ingress = SyncingIngress(Arc::new(AtomicBool::new(false)));
    let (publisher, _) = tokio::sync::broadcast::channel(16);

    super::reconcile_once_with_context(
        &database.deployments(),
        &database.domains(),
        &cipher,
        &super::ReconciliationContext {
            runtime: &runtime,
            ingress: &ingress,
            source_build: &super::NoopSourceBuild,
            dns_verifier: &ValidDnsVerifier,
        },
        &super::StreamPublisher::new(publisher),
    )
    .await
    .unwrap();

    let domains = database
        .domains()
        .list(actor, service.id.as_str())
        .await
        .unwrap();
    let domain = domains.unwrap().into_iter().next().unwrap();
    assert_eq!(domain.dns_status, DnsVerificationStatus::Valid);
    assert!(domain.dns_checked_at.is_some());
}

impl ImageRuntime for FakeRuntime {
    fn runtime_ref(&self, deployment: &super::RuntimeDeployment) -> String {
        format!("runtime-{}", deployment.id)
    }

    async fn start(
        &self,
        deployment: &super::RuntimeDeployment,
        _environment: Vec<String>,
    ) -> super::Result<String> {
        self.calls.lock().unwrap().push(deployment.id.to_string());
        Ok(format!("runtime-{}", deployment.id))
    }

    async fn inspect(
        &self,
        _deployment: &super::RuntimeDeployment,
        _runtime_ref: &str,
    ) -> super::Result<super::RuntimeObservation> {
        Ok(super::RuntimeObservation {
            owned: true,
            running: true,
            healthy: Some(false),
            health_failing: false,
        })
    }

    async fn stop(
        &self,
        _runtime_ref: &str,
        _service_id: &str,
        _generation: i64,
    ) -> super::Result<bool> {
        Ok(true)
    }

    async fn logs(&self, _runtime_ref: &str, _since: i64) -> super::Result<Vec<RuntimeLog>> {
        let mut logs = self.logs.lock().unwrap();
        Ok(if logs.is_empty() {
            vec![]
        } else {
            logs.remove(0)
        })
    }

    async fn reconcile_routes(
        &self,
        deployment: &super::RuntimeDeployment,
        _runtime_ref: &str,
        _environment: Vec<String>,
        _routes: Vec<IngressRoute>,
    ) -> super::Result<bool> {
        self.routed_local_images
            .lock()
            .unwrap()
            .push(deployment.local_image_id.clone());
        if self.routes_fail {
            return Err(super::Error::Runtime);
        }
        Ok(true)
    }
}

#[tokio::test]
async fn worker_restart_scan_recovers_preparing_deployment_without_restarting_runtime() {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let actor_id = database
        .users()
        .create("owner", "hash", DatabaseUserRole::User)
        .await
        .unwrap()
        .id;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    let input = ServiceInput::image(
        "web",
        "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some(8080),
        None,
        vec![],
    )
    .unwrap();
    let service = database
        .services()
        .create(
            ServiceActor {
                id: &actor_id,
                is_admin: false,
            },
            project.id.as_str(),
            input.configuration,
            Vec::<NewServiceVariable>::new(),
            None,
        )
        .await
        .unwrap();
    let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
        panic!("service must exist");
    };
    database
        .domains()
        .create(
            ignitify_db::DomainActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
            DomainName::new("app.example.com").unwrap(),
            ignitify_domain::DnsRecord::new(ignitify_domain::DnsRecordType::A, "203.0.113.10")
                .unwrap(),
        )
        .await
        .unwrap();
    let identity = age::x25519::Identity::generate().to_string();
    let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
    let variables_ciphertext = cipher.encrypt(b"{}").unwrap();
    let deployment = database
        .deployments()
        .create(
            DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
            ignitify_db::NewDeployment {
                idempotency_key: "deploy-1".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext,
            },
        )
        .await
        .unwrap();
    let ignitify_db::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let claimed = database.deployments().claim_next().await.unwrap().unwrap();
    database
        .deployments()
        .record_source_resolution(
            claimed.id.as_str(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            None,
        )
        .await
        .unwrap();
    database
        .deployments()
        .record_runtime_ref(claimed.id.as_str(), &format!("runtime-{}", claimed.id))
        .await
        .unwrap();
    let runtime = FakeRuntime {
        calls: Arc::new(Mutex::new(vec![])),
        routed_local_images: Arc::new(Mutex::new(vec![])),
        logs: Arc::new(Mutex::new(vec![
            vec![RuntimeLog {
                stream: "stdout".to_owned(),
                line: "starting application".to_owned(),
            }],
            vec![RuntimeLog {
                stream: "stdout".to_owned(),
                line: "application is ready".to_owned(),
            }],
        ])),
        routes_fail: false,
    };
    let (publisher, _) = tokio::sync::broadcast::channel(16);
    reconcile_once(
        &database.deployments(),
        &database.domains(),
        &cipher,
        &runtime,
        &FakeIngress,
        &super::StreamPublisher::new(publisher),
    )
    .await
    .unwrap();

    let events = database
        .deployments()
        .events(deployment.id.as_str())
        .await
        .unwrap();
    assert_eq!(
        events
            .into_iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        [
            "deployment.queued",
            "deployment.preparing",
            "deployment.running",
            "deployment.healthy"
        ]
    );
    assert!(runtime.calls.lock().unwrap().is_empty());
    let logs = database
        .deployments()
        .logs_after(deployment.id.as_str(), 0, 10)
        .await
        .unwrap();
    assert_eq!(
        logs.into_iter().map(|log| log.line).collect::<Vec<_>>(),
        ["starting application", "application is ready"]
    );
    assert_eq!(
        runtime.routed_local_images.lock().unwrap().as_slice(),
        [Some(
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()
        )]
    );
}

#[tokio::test]
async fn reconcile_marks_domains_failed_when_route_application_fails() {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let actor_id = database
        .users()
        .create("owner", "hash", DatabaseUserRole::User)
        .await
        .unwrap()
        .id;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    let service = database
        .services()
        .create(
            ServiceActor {
                id: &actor_id,
                is_admin: false,
            },
            project.id.as_str(),
            ServiceInput::image(
                "web",
                "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some(8080),
                None,
                vec![],
            )
            .unwrap()
            .configuration,
            Vec::<NewServiceVariable>::new(),
            None,
        )
        .await
        .unwrap();
    let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
        panic!("service must exist");
    };
    database
        .domains()
        .create(
            ignitify_db::DomainActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
            DomainName::new("app.example.com").unwrap(),
            ignitify_domain::DnsRecord::new(ignitify_domain::DnsRecordType::A, "203.0.113.10")
                .unwrap(),
        )
        .await
        .unwrap();
    let identity = age::x25519::Identity::generate().to_string();
    let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
    let deployment = database
        .deployments()
        .create(
            DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
            ignitify_db::NewDeployment {
                idempotency_key: "deploy-1".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: cipher.encrypt(b"{}").unwrap(),
            },
        )
        .await
        .unwrap();
    let ignitify_db::CreateDeploymentOutcome::Created(_) = deployment else {
        panic!("deployment must be created");
    };
    let claimed = database.deployments().claim_next().await.unwrap().unwrap();
    database
        .deployments()
        .record_runtime_ref(claimed.id.as_str(), &format!("runtime-{}", claimed.id))
        .await
        .unwrap();
    let runtime = FakeRuntime {
        calls: Arc::new(Mutex::new(vec![])),
        routed_local_images: Arc::new(Mutex::new(vec![])),
        logs: Arc::new(Mutex::new(vec![])),
        routes_fail: true,
    };
    let (publisher, _) = tokio::sync::broadcast::channel(16);

    assert!(
        reconcile_once(
            &database.deployments(),
            &database.domains(),
            &cipher,
            &runtime,
            &FakeIngress,
            &super::StreamPublisher::new(publisher),
        )
        .await
        .is_err()
    );

    let domains = database
        .domains()
        .list(
            ignitify_db::DomainActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(domains[0].status, DomainStatus::Failed);
    assert_eq!(
        domains[0].last_error.as_deref(),
        Some("route reconciliation failed")
    );
}

#[tokio::test]
async fn deployment_snapshot_merges_project_variables_before_service_overrides() {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let actor_id = database
        .users()
        .create("owner", "hash", DatabaseUserRole::User)
        .await
        .unwrap()
        .id;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    let identity = age::x25519::Identity::generate().to_string();
    let service_control = ServiceControl::new(
        database.services(),
        database.projects(),
        identity.expose_secret(),
    )
    .unwrap();
    let service = service_control
        .create(
            ServiceActor {
                id: &actor_id,
                is_admin: false,
            },
            project.id.as_str(),
            ServiceInput::image(
                "web",
                "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                None,
                None,
                vec![ServiceVariableInput {
                    key: "SHARED".to_owned(),
                    value: "service".to_owned(),
                    is_secret: false,
                }],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let super::ServiceMutationOutcomeModel::Created(service) = service else {
        panic!("service must exist");
    };
    service_control
        .update_project_environment(
            ProjectActor {
                id: &actor_id,
                is_admin: false,
            },
            project.id.as_str(),
            vec![
                ProjectEnvironmentVariableInput {
                    key: "SHARED".to_owned(),
                    value: Some("project".to_owned()),
                    is_secret: false,
                },
                ProjectEnvironmentVariableInput {
                    key: "PROJECT_ONLY".to_owned(),
                    value: Some("available".to_owned()),
                    is_secret: false,
                },
            ],
        )
        .await
        .unwrap();
    let (control, _wake) =
        crate::ControlHandle::new(database.deployments(), identity.expose_secret()).unwrap();
    let submission = control
        .submit_deploy(
            DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            &service.id,
            "deploy-1",
        )
        .await
        .unwrap();
    let deployment = match submission {
        crate::DeploymentSubmission::Accepted(deployment) => deployment,
        _ => panic!("deployment must be accepted"),
    };
    let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
    let values = crate::deployment_values::decrypt_deployment_values(
        &cipher,
        &deployment.variables_ciphertext,
    )
    .unwrap();
    assert_eq!(values["SHARED"].as_str(), "service");
    assert_eq!(values["PROJECT_ONLY"].as_str(), "available");
}
