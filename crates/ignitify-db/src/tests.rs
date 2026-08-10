use chrono::Utc;
use ignitify_domain::{
    ApplicationBuilder, DnsRecord, DnsRecordType, DnsVerificationStatus, DomainName, ProjectInput,
    ProjectMemberRole, ServiceInput, ServiceSourceConfig, ServiceSpec, ServiceVariableInput,
};
use uuid::Uuid;

use crate::{
    ActivityActor, Database, DatabaseConfig, DomainActor, NewBackupS3Destination, NewProvider,
    NewRemoteBuilder, NewRemoteServer, NewServerCertificate, NewServiceVariable, NewUptimeMonitor,
    ProjectActor, ProjectRemoveOutcome, ProjectUpdateOutcome, ProviderAuthMode, ProviderKind,
    RemoteServerAgentHeartbeat, ServerSettingsUpdate, ServiceActor, ServiceMutationOutcome,
    UptimeCheckUpdate, UptimeMonitorUpdate,
};

async fn database() -> Database {
    Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap()
}

async fn user_id(database: &Database, username: &str) -> String {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, is_active, created_at) VALUES (?, ?, 'password-hash', 'user', 1, ?)",
    )
    .bind(&id)
    .bind(username)
    .bind(Utc::now().to_rfc3339())
    .execute(&database.pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn migrations_create_auth_storage() {
    let database = database().await;

    assert_eq!(database.users().count().await.unwrap(), 0);
    let settings = database.server_settings().get().await.unwrap();
    assert!(settings.https_enabled);
    assert!(settings.automatically_provision_ssl);
    assert!(settings.acme_email.is_empty());
    assert_eq!(settings.certificate_provider, "lets-encrypt");
    assert_eq!(settings.fallback_page_heading, "Application not found");
}

#[tokio::test]
async fn backup_s3_controls_and_run_history_are_durable() {
    let database = database().await;
    database
        .backup_destinations()
        .upsert_s3(NewBackupS3Destination {
            endpoint: "https://s3.example.test".to_owned(),
            region: "us-east-1".to_owned(),
            bucket: "ignitify-backups".to_owned(),
            prefix: "production".to_owned(),
            access_key_id_ciphertext: "access".to_owned(),
            secret_access_key_ciphertext: "secret".to_owned(),
            session_token_ciphertext: None,
            server_side_encryption: "AES256".to_owned(),
        })
        .await
        .unwrap();

    let destination = database
        .backup_destinations()
        .update_s3_controls(false, Some(48))
        .await
        .unwrap()
        .unwrap();
    assert!(!destination.enabled);
    assert_eq!(destination.schedule_interval_hours, Some(48));
    assert!(
        database
            .backup_destinations()
            .s3_connection()
            .await
            .unwrap()
            .is_none()
    );

    database
        .backup_destinations()
        .start_s3_run("run-1", "manual")
        .await
        .unwrap();
    database
        .backup_destinations()
        .finish_s3_run("run-1", true)
        .await
        .unwrap();
    let runs = database
        .backup_destinations()
        .list_s3_runs(10)
        .await
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "succeeded");
    assert_eq!(runs[0].message.as_deref(), Some("Backup completed"));
}

#[tokio::test]
async fn uptime_monitors_are_scoped_and_record_check_history() {
    let database = database().await;
    let owner_id = user_id(&database, "uptime-owner").await;
    let other_id = user_id(&database, "uptime-other").await;
    let created = database
        .uptime_monitors()
        .create(NewUptimeMonitor {
            user_id: owner_id.clone(),
            name: "Portal".to_owned(),
            target: "https://portal.example.com/health".to_owned(),
            kind: "http".to_owned(),
            interval_seconds: 60,
            enabled: true,
        })
        .await
        .unwrap();

    assert_eq!(
        database
            .uptime_monitors()
            .list_for_user(&owner_id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        database
            .uptime_monitors()
            .list_for_user(&other_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        database
            .uptime_monitors()
            .create(NewUptimeMonitor {
                user_id: owner_id.clone(),
                name: "Portal".to_owned(),
                target: "https://other.example.com".to_owned(),
                kind: "http".to_owned(),
                interval_seconds: 60,
                enabled: true,
            })
            .await,
        Err(crate::DatabaseError::UptimeMonitorNameConflict)
    ));

    database
        .uptime_monitors()
        .record_check(
            &created.id,
            &created.updated_at,
            UptimeCheckUpdate {
                status: "up".to_owned(),
                latency_ms: Some(42),
                last_error: None,
                checked_at: Utc::now().to_rfc3339(),
            },
        )
        .await
        .unwrap();
    let checked = database
        .uptime_monitors()
        .list_for_user(&owner_id)
        .await
        .unwrap();
    assert_eq!(checked[0].status, "up");
    assert_eq!(checked[0].latency_ms, Some(42));
    assert_eq!(checked[0].history.last().map(String::as_str), Some("up"));

    assert!(
        database
            .uptime_monitors()
            .update(
                &other_id,
                &created.id,
                UptimeMonitorUpdate {
                    name: "Attempt".to_owned(),
                    target: "https://other.example.com".to_owned(),
                    kind: "http".to_owned(),
                    interval_seconds: 60,
                    enabled: true,
                },
            )
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn backup_s3_destination_keeps_credentials_internal_and_replaces_configuration() {
    let database = database().await;
    assert!(database.backup_destinations().s3().await.unwrap().is_none());

    let created = database
        .backup_destinations()
        .upsert_s3(NewBackupS3Destination {
            endpoint: "https://account.r2.cloudflarestorage.com".to_owned(),
            region: "auto".to_owned(),
            bucket: "ignitify-backups".to_owned(),
            prefix: "production/control-plane".to_owned(),
            access_key_id_ciphertext: "encrypted-access-key".to_owned(),
            secret_access_key_ciphertext: "encrypted-secret-key".to_owned(),
            session_token_ciphertext: Some("encrypted-session-token".to_owned()),
            server_side_encryption: "AES256".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(created.bucket, "ignitify-backups");
    assert_eq!(created.server_side_encryption, "AES256");

    let connection = database
        .backup_destinations()
        .s3_connection()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.access_key_id_ciphertext, "encrypted-access-key");
    assert_eq!(
        connection.session_token_ciphertext.as_deref(),
        Some("encrypted-session-token")
    );

    let updated = database
        .backup_destinations()
        .upsert_s3(NewBackupS3Destination {
            endpoint: "https://s3.ap-southeast-1.amazonaws.com".to_owned(),
            region: "ap-southeast-1".to_owned(),
            bucket: "ignitify-backups".to_owned(),
            prefix: String::new(),
            access_key_id_ciphertext: "replacement-access-key".to_owned(),
            secret_access_key_ciphertext: "replacement-secret-key".to_owned(),
            session_token_ciphertext: None,
            server_side_encryption: "AES256".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(updated.region, "ap-southeast-1");
    assert_eq!(updated.prefix, "");
    assert!(database.backup_destinations().delete_s3().await.unwrap());
    assert!(database.backup_destinations().s3().await.unwrap().is_none());
}

#[tokio::test]
async fn server_settings_and_encrypted_certificate_records_are_durable() {
    let database = database().await;
    let updated = database
        .server_settings()
        .update(ServerSettingsUpdate {
            application_domain_suffix: "apps.example.com".to_owned(),
            https_enabled: true,
            automatically_provision_ssl: true,
            acme_email: "ops@apps.example.com".to_owned(),
            dns_record_type: "a".to_owned(),
            dns_record_target: "203.0.113.10".to_owned(),
            fallback_page_heading: "This app is unavailable".to_owned(),
            fallback_page_message: "Check the hostname and try again.".to_owned(),
            certificate_provider: "lets-encrypt".to_owned(),
            custom_certificate_id: None,
            concurrent_builds: 4,
        })
        .await
        .unwrap();
    assert_eq!(updated.application_domain_suffix, "apps.example.com");
    assert_eq!(updated.acme_email, "ops@apps.example.com");
    assert_eq!(updated.fallback_page_heading, "This app is unavailable");
    assert_eq!(updated.concurrent_builds, 4);

    let certificate = database
        .server_settings()
        .create_certificate(NewServerCertificate {
            name: "Production wildcard".to_owned(),
            certificate_file_name: "production.crt".to_owned(),
            private_key_file_name: "production.key".to_owned(),
            certificate_ciphertext: "encrypted-certificate".to_owned(),
            private_key_ciphertext: "encrypted-private-key".to_owned(),
        })
        .await
        .unwrap();
    assert!(
        database
            .server_settings()
            .certificate_exists(&certificate.id)
            .await
            .unwrap()
    );
    let stored = database
        .server_settings()
        .list_certificates()
        .await
        .unwrap();
    assert_eq!(stored[0].certificate_ciphertext, "encrypted-certificate");
    assert_eq!(stored[0].private_key_ciphertext, "encrypted-private-key");
}

#[tokio::test]
async fn remote_builder_default_and_secrets_are_durable() {
    let database = database().await;
    let first = database
        .remote_builders()
        .create(NewRemoteBuilder {
            name: "Build cluster A".to_owned(),
            endpoint: "tcp://builder-a.example.com:1234".to_owned(),
            registry_repository: "registry.example.com/ignitify/builds".to_owned(),
            tls_server_name: Some("builder-a.example.com".to_owned()),
            ca_certificate_ciphertext: "encrypted-ca-a".to_owned(),
            client_certificate_ciphertext: "encrypted-cert-a".to_owned(),
            client_key_ciphertext: "encrypted-key-a".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();
    let second = database
        .remote_builders()
        .create(NewRemoteBuilder {
            name: "Build cluster B".to_owned(),
            endpoint: "tcp://builder-b.example.com:1234".to_owned(),
            registry_repository: "registry.example.com/ignitify/builds".to_owned(),
            tls_server_name: None,
            ca_certificate_ciphertext: "encrypted-ca-b".to_owned(),
            client_certificate_ciphertext: "encrypted-cert-b".to_owned(),
            client_key_ciphertext: "encrypted-key-b".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();

    let records = database.remote_builders().list().await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].id, second.id);
    assert!(records[0].is_default);
    assert!(!records[1].is_default);

    let active = database.remote_builders().active().await.unwrap().unwrap();
    assert_eq!(active.id, second.id);
    assert_eq!(active.client_key_ciphertext, "encrypted-key-b");

    let restored = database
        .remote_builders()
        .set_default(&first.id)
        .await
        .unwrap()
        .unwrap();
    assert!(restored.is_default);
    assert_eq!(
        database
            .remote_builders()
            .active()
            .await
            .unwrap()
            .unwrap()
            .id,
        first.id
    );
    assert!(database.remote_builders().delete(&first.id).await.unwrap());
    assert!(database.remote_builders().active().await.unwrap().is_none());
}

#[tokio::test]
async fn remote_server_default_and_ssh_secrets_are_durable() {
    let database = database().await;
    let first = database
        .remote_servers()
        .create(NewRemoteServer {
            name: "Production VM".to_owned(),
            host: "production.example.com".to_owned(),
            port: 22,
            username: "ignitify".to_owned(),
            deploy_path: "/srv/ignitify".to_owned(),
            private_key_ciphertext: "encrypted-private-key-a".to_owned(),
            public_key_ciphertext: "encrypted-public-key-a".to_owned(),
            known_hosts_ciphertext: "encrypted-known-hosts-a".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();
    let second = database
        .remote_servers()
        .create(NewRemoteServer {
            name: "Staging VM".to_owned(),
            host: "staging.example.com".to_owned(),
            port: 2222,
            username: "deploy".to_owned(),
            deploy_path: "/opt/ignitify".to_owned(),
            private_key_ciphertext: "encrypted-private-key-b".to_owned(),
            public_key_ciphertext: "encrypted-public-key-b".to_owned(),
            known_hosts_ciphertext: "encrypted-known-hosts-b".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();

    let records = database.remote_servers().list().await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].id, second.id);
    assert!(records[0].is_default);

    let active = database.remote_servers().active().await.unwrap().unwrap();
    assert_eq!(active.host, "staging.example.com");
    assert_eq!(active.private_key_ciphertext, "encrypted-private-key-b");
    assert_eq!(active.public_key_ciphertext, "encrypted-public-key-b");
    assert_eq!(active.known_hosts_ciphertext, "encrypted-known-hosts-b");

    let restored = database
        .remote_servers()
        .set_default(&first.id)
        .await
        .unwrap()
        .unwrap();
    assert!(restored.is_default);
    assert!(database.remote_servers().delete(&first.id).await.unwrap());
    assert!(database.remote_servers().active().await.unwrap().is_none());
}

#[tokio::test]
async fn deployment_snapshots_keep_the_selected_destination() {
    let database = database().await;
    let owner_id = user_id(&database, "destination-owner").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Destination app").unwrap())
        .await
        .unwrap();
    let destination = database
        .remote_servers()
        .create(NewRemoteServer {
            name: "Production VM".to_owned(),
            host: "production.example.com".to_owned(),
            port: 22,
            username: "ignitify".to_owned(),
            deploy_path: "/srv/ignitify".to_owned(),
            private_key_ciphertext: "encrypted-private-key".to_owned(),
            public_key_ciphertext: "encrypted-public-key".to_owned(),
            known_hosts_ciphertext: "encrypted-known-hosts".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();
    let mut input = ServiceInput::image(
        "web",
        "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some(80),
        None,
        vec![],
    )
    .unwrap();
    input.configuration.deployment_destination_id = Some(destination.id.clone());
    let service = database
        .services()
        .create(
            ServiceActor {
                id: &owner_id,
                is_admin: false,
            },
            project.id.as_str(),
            input.configuration,
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = service else {
        panic!("service must be created");
    };
    assert_eq!(
        service.deployment_destination_id.as_deref(),
        Some(destination.id.as_str())
    );

    let deployment = database
        .deployments()
        .create(
            crate::DeploymentActor {
                id: &owner_id,
                is_admin: false,
            },
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "destination-snapshot".to_owned(),
                requested_by_user_id: owner_id.clone(),
                spec: service.spec,
                source_config: service.source_config,
                deployment_destination_id: service.deployment_destination_id.clone(),
                source_revision: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    assert_eq!(
        deployment.deployment_destination_id.as_deref(),
        Some(destination.id.as_str())
    );
    assert!(matches!(
        database.remote_servers().delete(&destination.id).await,
        Err(crate::DatabaseError::RemoteServerInUse)
    ));
}

#[tokio::test]
async fn remote_server_agent_records_heartbeats_and_marks_stale_hosts_offline() {
    let database = database().await;
    let server = database
        .remote_servers()
        .create(NewRemoteServer {
            name: "Monitoring VM".to_owned(),
            host: "monitoring.example.com".to_owned(),
            port: 22,
            username: "ignitify".to_owned(),
            deploy_path: "/srv/ignitify".to_owned(),
            private_key_ciphertext: "encrypted-private-key".to_owned(),
            public_key_ciphertext: "encrypted-public-key".to_owned(),
            known_hosts_ciphertext: "encrypted-known-hosts".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();
    let agents = database.remote_server_agents();
    agents
        .install(&server.id, "hashed-agent-token")
        .await
        .unwrap();
    agents
        .record_heartbeat(
            &server.id,
            &RemoteServerAgentHeartbeat {
                version: "0.1.0".to_owned(),
                cpu_usage_percentage: Some(25.0),
                cpu_cores: Some(2),
                memory_used_bytes: Some(100),
                memory_total_bytes: Some(200),
                disk_used_bytes: Some(300),
                disk_total_bytes: Some(400),
                docker_containers: Some(3),
                docker_running_containers: Some(2),
                reported_at: Utc::now().to_rfc3339(),
            },
        )
        .await
        .unwrap();
    let online = agents.get(&server.id).await.unwrap().unwrap();
    assert_eq!(online.status, "online");
    assert_eq!(online.docker_running_containers, Some(2));

    agents
        .mark_stale(&(Utc::now() + chrono::Duration::seconds(1)).to_rfc3339())
        .await
        .unwrap();
    let offline = agents.get(&server.id).await.unwrap().unwrap();
    assert_eq!(offline.status, "offline");
    assert_eq!(
        offline.last_error.as_deref(),
        Some("agent heartbeat timed out")
    );
}

#[tokio::test]
async fn provider_repository_stores_encrypted_metadata_and_handles_conflicts() {
    let database = database().await;
    let actor_id = user_id(&database, "owner").await;
    let provider = database
        .providers()
        .create(
            &actor_id,
            NewProvider {
                name: "GitLab Cloud".to_owned(),
                kind: ProviderKind::Gitlab,
                auth_mode: ProviderAuthMode::OAuth,
                base_url: "https://gitlab.com".to_owned(),
                internal_url: None,
                redirect_uri: Some(
                    "https://ignitify.example.com/api/providers/gitlab/callback".to_owned(),
                ),
                client_id: Some("client-id".to_owned()),
                application_id: None,
                installation_id: None,
                group_names: None,
                username: Some("deploy".to_owned()),
                credentials_ciphertext: "age-encrypted-credentials".to_owned(),
            },
        )
        .await
        .unwrap();

    assert_eq!(database.providers().list().await.unwrap().len(), 1);
    assert_eq!(provider.kind, ProviderKind::Gitlab);
    let verified = database
        .providers()
        .mark_verified(&provider.id)
        .await
        .unwrap()
        .unwrap();
    assert!(verified.last_verified_at.is_some());
    assert!(matches!(
        database
            .providers()
            .create(
                &actor_id,
                NewProvider {
                    name: "GitLab Cloud".to_owned(),
                    kind: ProviderKind::Git,
                    auth_mode: ProviderAuthMode::Token,
                    base_url: "https://git.example.com".to_owned(),
                    internal_url: None,
                    redirect_uri: None,
                    client_id: None,
                    application_id: None,
                    installation_id: None,
                    group_names: None,
                    username: None,
                    credentials_ciphertext: "another-token".to_owned(),
                },
            )
            .await,
        Err(crate::DatabaseError::ProviderNameConflict)
    ));

    assert!(database.providers().delete(&provider.id).await.unwrap());
    assert!(!database.providers().delete(&provider.id).await.unwrap());
}

#[tokio::test]
async fn project_bootstrap_creates_owner_and_production_environment() {
    let database = database().await;
    let actor_id = user_id(&database, "owner").await;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Control Plane").unwrap())
        .await
        .unwrap();

    let owner_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND user_id = ? AND role = 'owner'",
    )
    .bind(project.id.as_str())
    .bind(&actor_id)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    let environment_count = database
        .environments()
        .count_for_project(project.id.as_str())
        .await
        .unwrap();
    let default_name = database
        .environments()
        .default_name_for_project(project.id.as_str())
        .await
        .unwrap();

    assert_eq!(
        (owner_count, environment_count, default_name.as_deref()),
        (1, 1, Some("production"))
    );
}

#[tokio::test]
async fn activity_list_for_project_accepts_project_scope_bindings() {
    let database = database().await;
    let owner_id = user_id(&database, "owner").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Control Plane").unwrap())
        .await
        .unwrap();

    let activity = database
        .activity()
        .list_for_project(
            ActivityActor {
                id: &owner_id,
                is_admin: false,
            },
            project.id.as_str(),
            None,
            None,
        )
        .await
        .unwrap()
        .unwrap();

    assert!(!activity.is_empty());
}

#[tokio::test]
async fn project_authorization_rename_and_duplicate_name_are_enforced() {
    let database = database().await;
    let owner_id = user_id(&database, "owner").await;
    let other_id = user_id(&database, "other").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Control Plane").unwrap())
        .await
        .unwrap();
    let other = ProjectActor {
        id: &other_id,
        is_admin: false,
    };

    assert!(
        database
            .projects()
            .get(other, project.id.as_str())
            .await
            .unwrap()
            .is_none()
    );

    let owner = ProjectActor {
        id: &owner_id,
        is_admin: false,
    };
    let renamed = database
        .projects()
        .rename(
            owner.clone(),
            project.id.as_str(),
            ProjectInput::new("Platform").unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(renamed, ProjectUpdateOutcome::Updated(_)));

    database
        .projects()
        .create(&owner_id, ProjectInput::new("Other").unwrap())
        .await
        .unwrap();
    let conflict = database
        .projects()
        .rename(
            owner,
            project.id.as_str(),
            ProjectInput::new("Other").unwrap(),
        )
        .await;
    assert!(matches!(
        conflict,
        Err(crate::DatabaseError::ProjectNameConflict)
    ));
}

#[tokio::test]
async fn project_remove_requires_matching_owner_confirmation_and_cascades_children() {
    let database = database().await;
    let owner_id = user_id(&database, "owner").await;
    let editor_id = user_id(&database, "editor").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Control Plane").unwrap())
        .await
        .unwrap();
    database
        .projects()
        .add_member(project.id.as_str(), &editor_id, ProjectMemberRole::Editor)
        .await
        .unwrap();

    let owner = ProjectActor {
        id: &owner_id,
        is_admin: false,
    };
    let editor = ProjectActor {
        id: &editor_id,
        is_admin: false,
    };
    assert!(matches!(
        database
            .projects()
            .remove(owner.clone(), project.id.as_str(), "Wrong name")
            .await,
        Err(crate::DatabaseError::ProjectConfirmationMismatch)
    ));
    assert!(matches!(
        database
            .projects()
            .remove(editor, project.id.as_str(), "Control Plane")
            .await
            .unwrap(),
        ProjectRemoveOutcome::Forbidden
    ));

    assert!(matches!(
        database
            .projects()
            .remove(owner, project.id.as_str(), "Control Plane")
            .await
            .unwrap(),
        ProjectRemoveOutcome::Removed
    ));
    assert!(
        database
            .projects()
            .get(
                ProjectActor {
                    id: &owner_id,
                    is_admin: false,
                },
                project.id.as_str(),
            )
            .await
            .unwrap()
            .is_none()
    );
    let child_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM environments WHERE project_id = ?")
            .bind(project.id.as_str())
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(child_count, 0);
}

#[tokio::test]
async fn deployment_repository_enforces_idempotency_active_conflict_and_immutable_rollback() {
    let database = database().await;
    let actor_id = user_id(&database, "owner").await;
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
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = service else {
        panic!("service must be created");
    };
    let actor = crate::DeploymentActor {
        id: &actor_id,
        is_admin: false,
    };
    let first = database
        .deployments()
        .create(
            actor,
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "deploy-1".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec.clone(),
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: "ciphertext-1".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(first) = first else {
        panic!("first deployment must be created");
    };
    let repeated = database
        .deployments()
        .create(
            actor,
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "deploy-1".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec.clone(),
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: "different-ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Existing(repeated) = repeated else {
        panic!("same idempotency key must return existing deployment");
    };
    assert_eq!(repeated.id, first.id);
    let competing = database
        .deployments()
        .create(
            actor,
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "deploy-2".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec.clone(),
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: "ciphertext-2".to_owned(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        competing,
        crate::CreateDeploymentOutcome::ActiveConflict
    ));
    database
        .deployments()
        .transition(
            first.id.as_str(),
            ignitify_domain::DeploymentState::Preparing,
            None,
            None,
        )
        .await
        .unwrap();
    database
        .deployments()
        .transition(
            first.id.as_str(),
            ignitify_domain::DeploymentState::Running,
            Some("runtime-1"),
            None,
        )
        .await
        .unwrap();
    database
        .deployments()
        .transition(
            first.id.as_str(),
            ignitify_domain::DeploymentState::Healthy,
            Some("runtime-1"),
            None,
        )
        .await
        .unwrap();
    let rollback = database
        .deployments()
        .rollback(actor, first.id.as_str(), "rollback-1")
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(rollback) = rollback else {
        panic!("rollback must create a new deployment");
    };
    assert_eq!(
        (
            rollback.generation,
            rollback.spec,
            rollback.variables_ciphertext
        ),
        (first.generation + 1, first.spec, first.variables_ciphertext)
    );
}

#[tokio::test]
async fn deployment_retry_backoff_and_cancellation_are_durable() {
    let database = database().await;
    let actor_id = user_id(&database, "execution-owner").await;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Execution control").unwrap())
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
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = service else {
        panic!("service must be created");
    };
    let actor = crate::DeploymentActor {
        id: &actor_id,
        is_admin: false,
    };
    let deployment = database
        .deployments()
        .create(
            actor,
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "execution-control".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };

    let claimed = database.deployments().claim_next().await.unwrap().unwrap();
    assert_eq!(claimed.attempt_count, 1);
    let retry = database
        .deployments()
        .schedule_retry(claimed.id.as_str(), "runtime did not start", 3)
        .await
        .unwrap();
    assert!(matches!(retry, crate::RetrySchedule::Scheduled { .. }));
    let queued = database
        .deployments()
        .get(actor, deployment.id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(queued.state, ignitify_domain::DeploymentState::Queued);
    assert_eq!(queued.attempt_count, 1);
    assert!(queued.retry_after.is_some());

    let cancelled = database
        .deployments()
        .cancel(actor, deployment.id.as_str())
        .await
        .unwrap();
    let crate::CancelDeploymentOutcome::Cancelled(cancelled) = cancelled else {
        panic!("queued deployment must be cancelled");
    };
    assert_eq!(cancelled.state, ignitify_domain::DeploymentState::Stopped);
    assert!(cancelled.cancel_requested_at.is_some());
    assert!(database.deployments().claim_next().await.unwrap().is_none());
    let events = database
        .deployments()
        .events(deployment.id.as_str())
        .await
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.kind == "deployment.retry_scheduled")
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == "deployment.cancelled")
    );
}

#[tokio::test]
async fn service_repository_persists_source_configuration_separately_from_runtime_spec() {
    let database = database().await;
    let actor_id = user_id(&database, "source-owner").await;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Source project").unwrap())
        .await
        .unwrap();
    let mut input = ServiceInput::image(
        "web",
        "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some(80),
        None,
        vec![],
    )
    .unwrap();
    input.configuration.source_config = Some(ServiceSourceConfig {
        source: "application".to_owned(),
        template: None,
        setup_required: None,
        provider_id: Some("provider-1".to_owned()),
        repository: Some("acme/site".to_owned()),
        branch: Some("main".to_owned()),
        builder: Some(ApplicationBuilder::Railpack),
        dockerfile_path: None,
        build_command: None,
        output_directory: None,
    });
    let outcome = database
        .services()
        .create(
            ServiceActor {
                id: &actor_id,
                is_admin: false,
            },
            project.id.as_str(),
            input.configuration,
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = outcome else {
        panic!("service must be created");
    };
    assert_eq!(
        service
            .source_config
            .as_ref()
            .and_then(|config| config.repository.as_deref()),
        Some("acme/site")
    );
    assert_eq!(service.spec.kind().as_str(), "image");

    let deployment = database
        .deployments()
        .create(
            crate::DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "source-deploy".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec.clone(),
                source_config: service.source_config.clone(),
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let claimed = database.deployments().claim_next().await.unwrap().unwrap();
    let resolved_spec = ServiceSpec::image(
        "caddy@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        Some(80),
        None,
    )
    .unwrap();
    assert!(
        database
            .deployments()
            .record_source_resolution(
                claimed.id.as_str(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                Some(&resolved_spec),
            )
            .await
            .unwrap()
    );
    let stored = database
        .deployments()
        .get(
            crate::DeploymentActor {
                id: &deployment.requested_by_user_id,
                is_admin: false,
            },
            deployment.id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.source_config, service.source_config);
    assert_eq!(
        stored.source_revision.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        stored.local_image_id.as_deref(),
        Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    );
    assert_eq!(stored.spec, resolved_spec);
    database
        .deployments()
        .transition(
            stored.id.as_str(),
            ignitify_domain::DeploymentState::Failed,
            None,
            Some("test failure"),
        )
        .await
        .unwrap();
    let rollback = database
        .deployments()
        .rollback(
            crate::DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            stored.id.as_str(),
            "source-rollback",
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(rollback) = rollback else {
        panic!("rollback must be created");
    };
    assert_eq!(rollback.source_revision, stored.source_revision);
}

#[tokio::test]
async fn deployment_log_retention_keeps_newest_ten_thousand_rows() {
    let database = database().await;
    let actor_id = user_id(&database, "owner").await;
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
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = service else {
        panic!("service must be created");
    };
    let deployment = database
        .deployments()
        .create(
            crate::DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "retention".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let logs = (0..10_001)
        .map(|number| crate::NewDeploymentLog {
            stream: "stdout".to_owned(),
            line: number.to_string(),
        })
        .collect::<Vec<_>>();
    database
        .deployments()
        .append_logs(deployment.id.as_str(), &logs)
        .await
        .unwrap();
    let (count, oldest): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), MIN(sequence) FROM deployment_logs WHERE deployment_id = ?",
    )
    .bind(deployment.id.as_str())
    .fetch_one(&database.pool)
    .await
    .unwrap();

    assert_eq!((count, oldest), (10_000, 2));
}

#[tokio::test]
async fn domain_repository_enforces_hostname_uniqueness_role_and_confirmation() {
    let database = database().await;
    let owner_id = user_id(&database, "owner").await;
    let viewer_id = user_id(&database, "viewer").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    database
        .projects()
        .add_member(project.id.as_str(), &viewer_id, ProjectMemberRole::Viewer)
        .await
        .unwrap();
    let service = database
        .services()
        .create(
            ServiceActor {
                id: &owner_id,
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
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = service else {
        panic!("service must be created");
    };
    let owner = crate::DomainActor {
        id: &owner_id,
        is_admin: false,
    };
    let created = database
        .domains()
        .create(
            owner,
            service.id.as_str(),
            DomainName::new("app.example.com").unwrap(),
            DnsRecord::new(DnsRecordType::A, "203.0.113.10").unwrap(),
        )
        .await
        .unwrap();
    let crate::DomainMutationOutcome::Created(domain) = created else {
        panic!("domain must be created");
    };
    let activity = database
        .activity()
        .list_for_project(
            ActivityActor {
                id: &owner_id,
                is_admin: false,
            },
            project.id.as_str(),
            None,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(activity.iter().any(|entry| {
        entry.action == "domain.create" && entry.resource_id.as_deref() == Some(domain.id.as_str())
    }));
    let verification = database
        .domains()
        .request_dns_verification(owner, domain.id.as_str())
        .await
        .unwrap();
    let crate::DomainVerificationRequestOutcome::Requested(pending) = verification else {
        panic!("verification must be accepted");
    };
    assert_eq!(pending.dns_status, DnsVerificationStatus::Pending);
    assert_eq!(
        database
            .domains()
            .pending_dns_verifications()
            .await
            .unwrap()
            .len(),
        1
    );
    database
        .domains()
        .complete_dns_verification(domain.id.as_str(), DnsVerificationStatus::Valid, None)
        .await
        .unwrap();
    assert!(
        database
            .domains()
            .pending_dns_verifications()
            .await
            .unwrap()
            .is_empty()
    );
    let duplicate = database
        .domains()
        .create(
            owner,
            service.id.as_str(),
            DomainName::new("app.example.com").unwrap(),
            DnsRecord::new(DnsRecordType::A, "203.0.113.10").unwrap(),
        )
        .await;
    assert!(matches!(
        duplicate,
        Err(crate::DatabaseError::DomainNameConflict)
    ));
    let viewer = crate::DomainActor {
        id: &viewer_id,
        is_admin: false,
    };
    let viewer_remove = database
        .domains()
        .remove(viewer, domain.id.as_str(), "app.example.com")
        .await
        .unwrap();
    assert!(matches!(
        viewer_remove,
        crate::DomainMutationOutcome::Forbidden
    ));
    let mismatch = database
        .domains()
        .remove(owner, domain.id.as_str(), "other.example.com")
        .await;
    assert!(matches!(
        mismatch,
        Err(crate::DatabaseError::DomainConfirmationMismatch)
    ));
}

#[tokio::test]
async fn service_repository_enforces_role_updates_generation_and_audits_without_plaintext() {
    let database = database().await;
    let owner_id = user_id(&database, "owner").await;
    let viewer_id = user_id(&database, "viewer").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    database
        .projects()
        .add_member(project.id.as_str(), &viewer_id, ProjectMemberRole::Viewer)
        .await
        .unwrap();
    let input = ServiceInput::image(
        "web",
        "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Some(8080),
        None,
        vec![ServiceVariableInput {
            key: "TOKEN".to_owned(),
            value: "plain-secret".to_owned(),
            is_secret: true,
        }],
    )
    .unwrap();
    let variables = vec![NewServiceVariable {
        key: "TOKEN".to_owned(),
        is_secret: true,
        ciphertext: "armored-ciphertext".to_owned(),
    }];
    let owner = ServiceActor {
        id: &owner_id,
        is_admin: false,
    };
    let created = database
        .services()
        .create(owner, project.id.as_str(), input.configuration, variables)
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = created else {
        panic!("owner service create must succeed");
    };
    let viewer = ServiceActor {
        id: &viewer_id,
        is_admin: false,
    };
    let viewer_update = database
        .services()
        .update(
            viewer,
            service.id.as_str(),
            ServiceInput::image(
                "web",
                "nginx@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                None,
                None,
                vec![],
            )
            .unwrap()
            .configuration,
            vec![],
        )
        .await
        .unwrap();
    assert!(matches!(viewer_update, ServiceMutationOutcome::Forbidden));
    let updated = database
        .services()
        .update(
            owner,
            service.id.as_str(),
            ServiceInput::image(
                "web",
                "nginx@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                None,
                None,
                vec![],
            )
            .unwrap()
            .configuration,
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Updated(updated) = updated else {
        panic!("owner service update must succeed");
    };
    assert_eq!(updated.desired_generation, 2);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE resource_id = ? AND details_json IS NULL",
    )
    .bind(service.id.as_str())
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 2);
}

#[tokio::test]
async fn service_removal_requires_confirmation_and_cascades_stopped_records() {
    let database = database().await;
    let owner_id = user_id(&database, "owner").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Platform").unwrap())
        .await
        .unwrap();
    let owner = ServiceActor {
        id: &owner_id,
        is_admin: false,
    };
    let created = database
        .services()
        .create(
            owner,
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
            vec![],
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = created else {
        panic!("service must be created");
    };
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO deployments (
            id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
            variables_ciphertext, status, created_at
         ) VALUES (?, ?, 1, 'deployment-1', ?, '{}', '{}', 'healthy', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(service.id.as_str())
    .bind(&owner_id)
    .bind(&now)
    .execute(&database.pool)
    .await
    .unwrap();

    assert!(matches!(
        database
            .services()
            .remove(owner, service.id.as_str(), "web")
            .await,
        Err(crate::DatabaseError::ServiceHasActiveDeployment)
    ));

    sqlx::query("UPDATE deployments SET status = 'stopped' WHERE service_id = ?")
        .bind(service.id.as_str())
        .execute(&database.pool)
        .await
        .unwrap();
    database
        .domains()
        .create(
            DomainActor {
                id: &owner_id,
                is_admin: false,
            },
            service.id.as_str(),
            DomainName::new("web.example.com").unwrap(),
            DnsRecord::new(DnsRecordType::Cname, "edge.example.com").unwrap(),
        )
        .await
        .unwrap();

    let removed = database
        .services()
        .remove(owner, service.id.as_str(), "web")
        .await
        .unwrap();
    assert!(matches!(removed, ServiceMutationOutcome::Removed(_)));
    for table in ["services", "deployments", "domains", "service_variables"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&database.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} must cascade when removing the service");
    }
}
