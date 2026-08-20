use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use age::secrecy::ExposeSecret;
use ignitify_db::{
    Database, DatabaseConfig, DeploymentActor, DeploymentRecord, NewDeployment, NewServiceVariable,
    ProjectActor, ProjectRemoveOutcome, ServiceActor, ServiceMutationOutcome,
};
use ignitify_domain::{
    ApplicationBuilder, DeploymentState, ProjectInput, ServiceInput, ServiceSourceConfig,
    SupplyChainCheckStatus, SupplyChainEnforcement,
};

use super::{
    AgeCipher, Error, ImageRuntime, Ingress, RuntimeDeployment, RuntimeObservation, SourceBuild,
    StreamPublisher, process_claimed_deployment, reconcile_once, reconcile_once_with_source,
};
use crate::{ControlHandle, DeploymentSubmission, IngressRoute, RuntimeLog, SourceBuildOutput};

struct DeploymentContext {
    database: Database,
    actor_id: String,
    project_id: String,
    deployment: DeploymentRecord,
    cipher: AgeCipher,
}

async fn queued_image_deployment() -> DeploymentContext {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let actor_id = database
        .users()
        .create("owner", "hash", ignitify_db::UserRole::User)
        .await
        .unwrap()
        .id;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Resilience").unwrap())
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
        panic!("service must be created");
    };
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
            NewDeployment {
                idempotency_key: "resilience-deploy".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                supply_chain_report: None,
                variables_ciphertext: cipher.encrypt(b"{}").unwrap(),
            },
        )
        .await
        .unwrap();
    let ignitify_db::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let ignitify_db::DeploymentApprovalOutcome::Approved(deployment) = database
        .deployments()
        .approve(
            DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            deployment.id.as_str(),
        )
        .await
        .unwrap()
    else {
        panic!("deployment must be approved for worker resilience tests");
    };
    DeploymentContext {
        database,
        actor_id,
        project_id: project.id.to_string(),
        deployment,
        cipher,
    }
}

async fn queued_unresolved_compose_deployment() -> DeploymentContext {
    let database = Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let actor_id = database
        .users()
        .create("owner", "hash", ignitify_db::UserRole::User)
        .await
        .unwrap()
        .id;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Resilience").unwrap())
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
            ServiceInput::compose(
                "web",
                "services:\n  app:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "app",
                Some(8080),
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
        panic!("service must be created");
    };
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
            NewDeployment {
                idempotency_key: "unresolved-compose".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                supply_chain_report: None,
                variables_ciphertext: cipher.encrypt(b"{}").unwrap(),
            },
        )
        .await
        .unwrap();
    let ignitify_db::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let ignitify_db::DeploymentApprovalOutcome::Approved(deployment) = database
        .deployments()
        .approve(
            DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            deployment.id.as_str(),
        )
        .await
        .unwrap()
    else {
        panic!("deployment must be approved for worker resilience tests");
    };
    DeploymentContext {
        database,
        actor_id,
        project_id: project.id.to_string(),
        deployment,
        cipher,
    }
}

fn publisher() -> StreamPublisher {
    let (publisher, _) = tokio::sync::broadcast::channel(16);
    StreamPublisher::new(publisher)
}

struct ReadyIngress;

impl Ingress for ReadyIngress {
    fn route(
        &self,
        _service_id: &ignitify_domain::ServiceId,
        _domain_id: &ignitify_domain::DomainId,
        _hostname: &ignitify_domain::DomainName,
        _port: u32,
    ) -> super::Result<IngressRoute> {
        Ok(IngressRoute {
            labels: Default::default(),
            network: "none".to_owned(),
        })
    }
}

struct FailingSyncIngress;

impl Ingress for FailingSyncIngress {
    fn route(
        &self,
        _service_id: &ignitify_domain::ServiceId,
        _domain_id: &ignitify_domain::DomainId,
        _hostname: &ignitify_domain::DomainName,
        _port: u32,
    ) -> super::Result<IngressRoute> {
        unreachable!("the failed ingress sync must prevent route construction")
    }

    async fn reconcile(&self) -> super::Result<()> {
        Err(Error::Runtime)
    }
}

struct RecordingRuntime {
    start_calls: Arc<AtomicUsize>,
    inspect_calls: Arc<AtomicUsize>,
    stop_calls: Arc<AtomicUsize>,
    cancel_on_start: Option<(ignitify_db::DeploymentsRepository, String)>,
}

impl RecordingRuntime {
    fn new() -> Self {
        Self {
            start_calls: Arc::new(AtomicUsize::new(0)),
            inspect_calls: Arc::new(AtomicUsize::new(0)),
            stop_calls: Arc::new(AtomicUsize::new(0)),
            cancel_on_start: None,
        }
    }

    fn cancelling_on_start(
        deployments: ignitify_db::DeploymentsRepository,
        actor_id: String,
    ) -> Self {
        Self {
            cancel_on_start: Some((deployments, actor_id)),
            ..Self::new()
        }
    }
}

impl ImageRuntime for RecordingRuntime {
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String {
        format!("runtime-{}", deployment.id)
    }

    async fn start(
        &self,
        deployment: &RuntimeDeployment,
        _environment: Vec<String>,
    ) -> super::Result<String> {
        self.start_calls.fetch_add(1, Ordering::Relaxed);
        if let Some((deployments, actor_id)) = &self.cancel_on_start {
            deployments
                .cancel(
                    DeploymentActor {
                        id: actor_id,
                        is_admin: false,
                    },
                    deployment.id.as_str(),
                )
                .await?;
        }
        Ok(self.runtime_ref(deployment))
    }

    async fn inspect(
        &self,
        _deployment: &RuntimeDeployment,
        _runtime_ref: &str,
    ) -> super::Result<RuntimeObservation> {
        self.inspect_calls.fetch_add(1, Ordering::Relaxed);
        Ok(RuntimeObservation {
            owned: true,
            running: true,
            healthy: Some(true),
            health_failing: false,
        })
    }

    async fn stop(
        &self,
        _runtime_ref: &str,
        _service_id: &str,
        _generation: i64,
    ) -> super::Result<bool> {
        self.stop_calls.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }

    async fn logs(&self, _runtime_ref: &str, _since: i64) -> super::Result<Vec<RuntimeLog>> {
        Ok(vec![])
    }
}

struct CancellingSourceBuild {
    deployments: ignitify_db::DeploymentsRepository,
    actor_id: String,
}

struct UnexpectedSourceBuild;

impl SourceBuild for UnexpectedSourceBuild {
    async fn build(
        &self,
        _deployment: &DeploymentRecord,
        _logs: &super::DeploymentLogSink,
    ) -> super::Result<Option<SourceBuildOutput>> {
        panic!("stored rollback artifact should bypass source build");
    }
}

struct ResolvingSourceBuild;

impl SourceBuild for ResolvingSourceBuild {
    async fn build(
        &self,
        _deployment: &DeploymentRecord,
        _logs: &super::DeploymentLogSink,
    ) -> super::Result<Option<SourceBuildOutput>> {
        Ok(Some(SourceBuildOutput {
            source_revision: "a".repeat(40),
            local_image_id: Some(format!("sha256:{}", "b".repeat(64))),
            runtime_spec: None,
        }))
    }
}

impl SourceBuild for CancellingSourceBuild {
    async fn build(
        &self,
        deployment: &DeploymentRecord,
        _logs: &super::DeploymentLogSink,
    ) -> super::Result<Option<SourceBuildOutput>> {
        self.deployments
            .cancel(
                DeploymentActor {
                    id: &self.actor_id,
                    is_admin: false,
                },
                deployment.id.as_str(),
            )
            .await?;
        Ok(None)
    }
}

#[tokio::test]
async fn rollback_reuses_stored_source_artifact_without_rebuilding() {
    let context = queued_image_deployment().await;
    let claimed = context
        .database
        .deployments()
        .claim_next()
        .await
        .unwrap()
        .unwrap();
    let mut rollback = claimed.clone();
    rollback.rollback_of_deployment_id = Some("source-deployment".to_owned());
    rollback.source_revision = Some("a".repeat(40));
    rollback.local_image_id = Some(format!("sha256:{}", "b".repeat(64)));
    rollback.source_config = Some(ServiceSourceConfig {
        source: "application".to_owned(),
        template: None,
        setup_required: Some(false),
        provider_id: Some("provider-1".to_owned()),
        repository: Some("acme/site".to_owned()),
        branch: Some("main".to_owned()),
        builder: Some(ApplicationBuilder::Dockerfile),
        dockerfile_path: Some("Dockerfile".to_owned()),
        build_command: None,
        output_directory: None,
        auto_deploy: false,
    });

    process_claimed_deployment(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &RecordingRuntime::new(),
        &ReadyIngress,
        &UnexpectedSourceBuild,
        &publisher(),
        rollback,
    )
    .await
    .unwrap();

    let stored = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.state, DeploymentState::Healthy);
}

#[tokio::test]
async fn ingress_sync_failure_leaves_queued_deployment_unclaimed() {
    let context = queued_image_deployment().await;
    let runtime = RecordingRuntime::new();

    let result = reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &FailingSyncIngress,
        &publisher(),
    )
    .await;

    assert!(matches!(result, Err(Error::Runtime)));
    let deployment = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deployment.state, DeploymentState::Queued);
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn cancellation_during_source_build_prevents_runtime_start() {
    let context = queued_image_deployment().await;
    let runtime = RecordingRuntime::new();
    let source_build = CancellingSourceBuild {
        deployments: context.database.deployments(),
        actor_id: context.actor_id.clone(),
    };

    reconcile_once_with_source(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &source_build,
        &publisher(),
    )
    .await
    .unwrap();

    let stopping = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopping.state, DeploymentState::Stopping);
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 0);

    reconcile_once_with_source(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &source_build,
        &publisher(),
    )
    .await
    .unwrap();

    let stopped = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, DeploymentState::Stopped);
}

#[tokio::test]
async fn source_build_records_resolved_supply_chain_provenance_in_warning_mode() {
    let context = queued_image_deployment().await;
    let runtime = RecordingRuntime::new();

    reconcile_once_with_source(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &ResolvingSourceBuild,
        &publisher(),
    )
    .await
    .unwrap();

    let deployment = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    let report = deployment
        .supply_chain_report
        .expect("report must be recorded");

    assert_eq!(report.status, SupplyChainCheckStatus::Warning);
    assert_eq!(report.provenance.status, SupplyChainCheckStatus::Pass);
    assert_eq!(report.sbom.status, SupplyChainCheckStatus::Warning);
    assert_eq!(
        report.vulnerabilities.status,
        SupplyChainCheckStatus::Warning
    );
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn require_provenance_policy_blocks_unresolved_deployment_before_runtime_start() {
    let context = queued_unresolved_compose_deployment().await;
    context
        .database
        .deployments()
        .update_supply_chain_enforcement(SupplyChainEnforcement::RequireProvenance)
        .await
        .unwrap();
    let runtime = RecordingRuntime::new();

    reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &publisher(),
    )
    .await
    .unwrap();

    let deployment = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    let report = deployment
        .supply_chain_report
        .expect("blocking report must be retained");

    assert_eq!(deployment.state, DeploymentState::Failed);
    assert_eq!(
        deployment.failure_reason.as_deref(),
        Some("supply-chain policy requires resolved provenance")
    );
    assert_eq!(
        report.enforcement,
        SupplyChainEnforcement::RequireProvenance
    );
    assert_eq!(report.provenance.status, SupplyChainCheckStatus::Warning);
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn cancellation_during_runtime_start_defers_observation_until_stop_reconciliation() {
    let context = queued_image_deployment().await;
    let runtime = RecordingRuntime::cancelling_on_start(
        context.database.deployments(),
        context.actor_id.clone(),
    );

    reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &publisher(),
    )
    .await
    .unwrap();

    let stopping = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopping.state, DeploymentState::Stopping);
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 1);
    assert_eq!(runtime.inspect_calls.load(Ordering::Relaxed), 0);

    reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &publisher(),
    )
    .await
    .unwrap();

    let stopped = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, DeploymentState::Stopped);
    assert_eq!(runtime.stop_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn worker_restart_reconciles_running_deployment_without_second_start() {
    let context = queued_image_deployment().await;
    let claimed = context
        .database
        .deployments()
        .claim_next()
        .await
        .unwrap()
        .unwrap();
    let runtime_ref = format!("runtime-{}", claimed.id);
    context
        .database
        .deployments()
        .record_runtime_ref(claimed.id.as_str(), &runtime_ref)
        .await
        .unwrap();
    context
        .database
        .deployments()
        .transition(
            claimed.id.as_str(),
            DeploymentState::Running,
            Some(&runtime_ref),
            None,
        )
        .await
        .unwrap();
    let runtime = RecordingRuntime::new();

    reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &publisher(),
    )
    .await
    .unwrap();

    let deployment = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deployment.state, DeploymentState::Healthy);
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 0);
    assert_eq!(runtime.inspect_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn worker_restart_finishes_stopping_deployment_without_starting_runtime() {
    let context = queued_image_deployment().await;
    let claimed = context
        .database
        .deployments()
        .claim_next()
        .await
        .unwrap()
        .unwrap();
    let runtime_ref = format!("runtime-{}", claimed.id);
    context
        .database
        .deployments()
        .record_runtime_ref(claimed.id.as_str(), &runtime_ref)
        .await
        .unwrap();
    context
        .database
        .deployments()
        .cancel(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            claimed.id.as_str(),
        )
        .await
        .unwrap();
    let runtime = RecordingRuntime::new();

    reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &publisher(),
    )
    .await
    .unwrap();

    let stopped = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            context.deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, DeploymentState::Stopped);
    assert_eq!(runtime.start_calls.load(Ordering::Relaxed), 0);
    assert_eq!(runtime.stop_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn stopping_a_healthy_compose_deployment_releases_service_and_project() {
    let context = queued_unresolved_compose_deployment().await;
    let claimed = context
        .database
        .deployments()
        .claim_next()
        .await
        .unwrap()
        .unwrap();
    let runtime_ref = format!("ignitify-{}-g{}", claimed.service_id, claimed.generation);
    context
        .database
        .deployments()
        .record_runtime_ref(claimed.id.as_str(), &runtime_ref)
        .await
        .unwrap();
    context
        .database
        .deployments()
        .transition(
            claimed.id.as_str(),
            DeploymentState::Running,
            Some(&runtime_ref),
            None,
        )
        .await
        .unwrap();
    context
        .database
        .deployments()
        .transition(
            claimed.id.as_str(),
            DeploymentState::Healthy,
            Some(&runtime_ref),
            None,
        )
        .await
        .unwrap();

    let identity = age::x25519::Identity::generate().to_string();
    let (control, _wake) =
        ControlHandle::new(context.database.deployments(), identity.expose_secret()).unwrap();
    let stopping = control
        .submit_stop(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            claimed.service_id.as_str(),
        )
        .await
        .unwrap();
    let stopping = match stopping {
        DeploymentSubmission::Accepted(stopping) => stopping,
        DeploymentSubmission::Existing(_) => panic!("healthy deployment stop was already active"),
        DeploymentSubmission::Missing => panic!("healthy deployment service was missing"),
        DeploymentSubmission::Forbidden => panic!("healthy deployment stop was forbidden"),
        DeploymentSubmission::ActiveConflict => {
            panic!("healthy deployment stop reported no active deployment")
        }
    };
    assert_eq!(stopping.state, DeploymentState::Stopping);

    let runtime = RecordingRuntime::new();
    reconcile_once(
        &context.database.deployments(),
        &context.database.domains(),
        &context.cipher,
        &runtime,
        &ReadyIngress,
        &publisher(),
    )
    .await
    .unwrap();

    let stopped = context
        .database
        .deployments()
        .get(
            DeploymentActor {
                id: &context.actor_id,
                is_admin: false,
            },
            claimed.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, DeploymentState::Stopped);
    assert_eq!(runtime.stop_calls.load(Ordering::Relaxed), 1);

    let removed_service = context
        .database
        .services()
        .remove(
            ServiceActor {
                id: &context.actor_id,
                is_admin: false,
            },
            claimed.service_id.as_str(),
            "web",
        )
        .await
        .unwrap();
    assert!(matches!(
        removed_service,
        ServiceMutationOutcome::Removed(_)
    ));
    let removed_project = context
        .database
        .projects()
        .remove(
            ProjectActor {
                id: &context.actor_id,
                is_admin: false,
            },
            &context.project_id,
            "Resilience",
        )
        .await
        .unwrap();
    assert_eq!(removed_project, ProjectRemoveOutcome::Removed);
}
