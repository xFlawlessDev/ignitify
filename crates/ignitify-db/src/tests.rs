use chrono::{Duration, Utc};
use ignitify_domain::{
    ApplicationBuilder, DnsRecord, DnsRecordType, DnsVerificationStatus, DomainName, ProjectInput,
    ProjectMemberRole, ServiceInput, ServiceSourceConfig, ServiceSpec, ServiceVariableInput,
    evaluate_supply_chain_report,
};
use uuid::Uuid;

use crate::{
    ActivityActor, Database, DatabaseConfig, DomainActor, NewBackupS3Destination,
    NewNotificationChannel, NewProvider, NewRemoteBuilder, NewRemoteServer, NewServerCertificate,
    NewServiceVariable, NewUptimeMonitor, ProjectActor, ProjectRemoveOutcome, ProjectUpdateOutcome,
    ProviderAuthMode, ProviderKind, RemoteServerAgentHeartbeat, ServerSettingsUpdate, ServiceActor,
    ServiceMutationOutcome, UPTIME_HISTORY_MAX_ROWS, UptimeCheckUpdate, UptimeMonitorUpdate,
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
    for table in [
        "deployments",
        "deployment_events",
        "deployment_logs",
        "audit_logs",
        "notification_deliveries",
    ] {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = 'correlation_id'"
        ))
        .fetch_one(&database.pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "{table} must retain correlation_id");
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('deployments') WHERE name = 'supply_chain_report_json'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "deployments must retain supply-chain reports");
    for column in [
        "approval_status",
        "approval_requested_at",
        "approved_by_user_id",
        "approved_at",
    ] {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM pragma_table_info('deployments') WHERE name = '{column}'"
        ))
        .fetch_one(&database.pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "deployments must retain {column}");
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'uptime_monitor_checks'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "uptime check history must be durable");
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
async fn notification_channels_encrypt_connection_configuration_and_deduplicate_deliveries() {
    let database = database().await;
    let channel = database
        .notification_channels()
        .create(NewNotificationChannel {
            name: "Operations Telegram".to_owned(),
            kind: "telegram".to_owned(),
            enabled: true,
            event_types: vec!["deployment.healthy".to_owned(), "backup.failed".to_owned()],
            configuration_summary: serde_json::json!({ "chat_id": "-100123" }),
            configuration_ciphertext: "encrypted-secret".to_owned(),
        })
        .await
        .unwrap();

    assert_eq!(channel.event_types, ["deployment.healthy", "backup.failed"]);
    assert!(channel.configuration_summary.get("bot_token").is_none());
    let connections = database
        .notification_channels()
        .enabled_for_event("deployment.healthy")
        .await
        .unwrap();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].configuration_ciphertext, "encrypted-secret");
    assert!(
        database
            .notification_channels()
            .claim_delivery(&channel.id, "deployment", "42", "deployment.healthy")
            .await
            .unwrap()
    );
    assert!(
        !database
            .notification_channels()
            .claim_delivery(&channel.id, "deployment", "42", "deployment.healthy")
            .await
            .unwrap()
    );
    assert!(
        database
            .notification_channels()
            .claim_delivery_with_correlation(
                &channel.id,
                "deployment",
                "deployment/deployment-1/event/7",
                "deployment.healthy",
                Some("correlation-1"),
            )
            .await
            .unwrap()
    );
    let correlated_delivery = database
        .notification_channels()
        .list_deliveries(10)
        .await
        .unwrap()
        .into_iter()
        .find(|delivery| delivery.source_id == "deployment/deployment-1/event/7")
        .unwrap();
    assert_eq!(
        correlated_delivery.correlation_id.as_deref(),
        Some("correlation-1")
    );
    assert!(
        database
            .notification_channels()
            .claim_delivery(&channel.id, "remote", "event-1", "remote_agent.offline")
            .await
            .unwrap()
    );
    assert!(
        !database
            .notification_channels()
            .claim_delivery(&channel.id, "remote", "event-1", "remote_agent.offline")
            .await
            .unwrap()
    );
    assert!(
        database
            .notification_channels()
            .claim_delivery(
                &channel.id,
                "operations",
                "backup.stale:1:raised",
                "operations.alert",
            )
            .await
            .unwrap()
    );
    assert!(
        !database
            .notification_channels()
            .claim_delivery(
                &channel.id,
                "operations",
                "backup.stale:1:raised",
                "operations.alert",
            )
            .await
            .unwrap()
    );
    database
        .notification_channels()
        .finish_delivery(&channel.id, "deployment", "42", "deployment.healthy", true)
        .await
        .unwrap();
    let deliveries = database
        .notification_channels()
        .list_deliveries(100)
        .await
        .unwrap();
    assert_eq!(deliveries.len(), 4);
    let deployment = deliveries
        .iter()
        .find(|delivery| delivery.source_kind == "deployment" && delivery.source_id == "42")
        .unwrap();
    assert_eq!(deployment.channel_name, "Operations Telegram");
    assert_eq!(deployment.status, "succeeded");
    assert_eq!(deployment.attempt_count, 0);
    assert_eq!(deployment.message.as_deref(), Some("Delivered"));
    assert!(deployment.completed_at.is_some());
    assert_eq!(
        database
            .notification_channels()
            .list_deliveries(1)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        database
            .notification_channels()
            .delete(&channel.id)
            .await
            .unwrap()
    );
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

    let mut expected_updated_at = checked[0].updated_at.clone();
    for (index, status) in ["up", "up", "up", "down"].into_iter().enumerate() {
        let checked_at = (Utc::now() - Duration::minutes(i64::from(4 - index as i32))).to_rfc3339();
        assert!(
            database
                .uptime_monitors()
                .record_check(
                    &created.id,
                    &expected_updated_at,
                    UptimeCheckUpdate {
                        status: status.to_owned(),
                        latency_ms: Some(20),
                        last_error: (status == "down").then(|| "connection failed".to_owned()),
                        checked_at: checked_at.clone(),
                    },
                )
                .await
                .unwrap()
        );
        expected_updated_at = checked_at;
    }
    let history = database
        .uptime_monitors()
        .history_for_user(&owner_id, &created.id, 24, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.checks.len(), 5);
    assert_eq!(history.summary.failed_checks, 1);
    assert_eq!(history.summary.status, "exhausted");
    assert_eq!(
        database
            .uptime_monitors()
            .budget_breached_count()
            .await
            .unwrap(),
        1
    );

    let old_check_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let mut transaction = database.pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO uptime_monitor_checks
         (id, monitor_id, status, latency_ms, error, checked_at)
         VALUES (?, ?, 'down', NULL, 'connection failed', ?)",
    )
    .bind(&old_check_id)
    .bind(&created.id)
    .bind((now - Duration::days(UPTIME_HISTORY_MAX_ROWS / 30)).to_rfc3339())
    .execute(&mut *transaction)
    .await
    .unwrap();
    for index in 0..=UPTIME_HISTORY_MAX_ROWS {
        sqlx::query(
            "INSERT INTO uptime_monitor_checks
             (id, monitor_id, status, latency_ms, error, checked_at)
             VALUES (?, ?, 'up', 20, NULL, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&created.id)
        .bind((now - Duration::seconds(index)).to_rfc3339())
        .execute(&mut *transaction)
        .await
        .unwrap();
    }
    transaction.commit().await.unwrap();
    assert!(
        database
            .uptime_monitors()
            .record_check(
                &created.id,
                &expected_updated_at,
                UptimeCheckUpdate {
                    status: "up".to_owned(),
                    latency_ms: Some(20),
                    last_error: None,
                    checked_at: Utc::now().to_rfc3339(),
                },
            )
            .await
            .unwrap()
    );
    let retained_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uptime_monitor_checks WHERE monitor_id = ?")
            .bind(&created.id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(retained_count, UPTIME_HISTORY_MAX_ROWS);
    let old_check_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uptime_monitor_checks WHERE id = ?")
            .bind(&old_check_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(old_check_count, 0);

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
            control_plane_domain: "console.example.com".to_owned(),
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
    assert_eq!(updated.control_plane_domain, "console.example.com");
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
            None,
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
                supply_chain_report: None,
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
    let events = agents.notification_events(10).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].server_id, server.id);
    assert_eq!(events[0].kind, "remote_agent.offline");

    agents
        .record_authentication_failure(&server.id)
        .await
        .unwrap();
    agents
        .record_authentication_failure(&server.id)
        .await
        .unwrap();
    assert_eq!(agents.notification_events(10).await.unwrap().len(), 1);
    agents
        .record_authentication_failure(&server.id)
        .await
        .unwrap();
    let events = agents.notification_events(10).await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(
        events
            .iter()
            .any(|event| event.kind == "remote_server.authentication_failed")
    );
    agents
        .record_authentication_failure(&server.id)
        .await
        .unwrap();
    assert_eq!(agents.notification_events(10).await.unwrap().len(), 2);
    agents
        .finish_notification_event(&events[0].id)
        .await
        .unwrap();
    let pending = agents.notification_events(10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_ne!(pending[0].id, events[0].id);
}

#[tokio::test]
async fn operations_summary_aggregates_safe_runtime_signals() {
    let database = database().await;
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO backup_s3_destination
            (id, endpoint, region, bucket, prefix, access_key_id_ciphertext,
             secret_access_key_ciphertext, server_side_encryption, enabled,
             schedule_interval_hours, created_at, updated_at)
         VALUES (1, 'https://s3.example.test', 'us-east-1', 'backups', 'ignitify',
                 'encrypted-access', 'encrypted-secret', 'AES256', 1, 24, ?, ?)",
    )
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO backup_s3_run (id, trigger, status, started_at, completed_at)
         VALUES ('scheduled-1', 'scheduled', 'succeeded', ?, ?)",
    )
    .bind((now - chrono::Duration::hours(1)).to_rfc3339())
    .bind((now - chrono::Duration::hours(1)).to_rfc3339())
    .execute(&database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO server_settings
            (id, server_domain, https_enabled, automatically_provision_ssl,
             certificate_provider, concurrent_builds, updated_at)
         VALUES (1, '', 1, 1, 'lets-encrypt', 2, ?)
         ON CONFLICT(id) DO UPDATE SET https_enabled = 1, certificate_provider = 'lets-encrypt'",
    )
    .bind(now.to_rfc3339())
    .execute(&database.pool)
    .await
    .unwrap();

    let summary = database.operations().summary().await.unwrap();
    assert_eq!(summary.deployments.queued_count, 0);
    assert_eq!(summary.backup.schedule_interval_hours, Some(24));
    assert_eq!(
        summary
            .backup
            .latest_scheduled_run
            .as_ref()
            .map(|run| run.status.as_str()),
        Some("succeeded")
    );
    assert!(summary.certificates.https_enabled);
    assert_eq!(summary.remote_agents.server_count, 0);
}

#[tokio::test]
async fn operational_alert_transitions_are_deduplicated_and_rearm_after_resolution() {
    let database = database().await;
    let operations = database.operations();

    assert!(
        operations
            .transition_alert("backup.stale", false)
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        operations
            .transition_alert("backup.stale", true)
            .await
            .unwrap(),
        Some(crate::OperationalAlertTransition::Raised { generation: 1 })
    ));
    assert!(
        operations
            .transition_alert("backup.stale", true)
            .await
            .unwrap()
            .is_none()
    );
    let pending = operations.pending_alert_events(10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].kind, "raised");
    operations
        .finish_alert_event("backup.stale", 1, "raised")
        .await
        .unwrap();
    assert!(
        operations
            .pending_alert_events(10)
            .await
            .unwrap()
            .is_empty()
    );

    assert!(matches!(
        operations
            .transition_alert("backup.stale", false)
            .await
            .unwrap(),
        Some(crate::OperationalAlertTransition::Resolved { generation: 1 })
    ));
    assert!(matches!(
        operations
            .transition_alert("backup.stale", true)
            .await
            .unwrap(),
        Some(crate::OperationalAlertTransition::Raised { generation: 2 })
    ));
    let pending = operations.pending_alert_events(10).await.unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().any(|event| event.kind == "resolved"));
    assert!(pending.iter().any(|event| event.kind == "raised"));
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
            None,
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
    let report = evaluate_supply_chain_report(
        &service.spec,
        None,
        None,
        None,
        "2026-08-14T00:00:00Z".to_owned(),
    );
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
                supply_chain_report: Some(report.clone()),
                variables_ciphertext: "ciphertext-1".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(first) = first else {
        panic!("first deployment must be created");
    };
    assert!(!first.correlation_id.is_empty());
    assert_eq!(first.supply_chain_report, Some(report.clone()));
    let queued_events = database
        .deployments()
        .events(first.id.as_str())
        .await
        .unwrap();
    assert_eq!(queued_events.len(), 1);
    assert_eq!(
        queued_events[0].event_id,
        format!(
            "deployment/{}/event/{}",
            first.id, queued_events[0].sequence
        )
    );
    assert_eq!(queued_events[0].correlation_id, first.correlation_id);
    let logs = database
        .deployments()
        .append_logs(
            first.id.as_str(),
            &[crate::NewDeploymentLog {
                stream: "system".to_owned(),
                line: "correlated lifecycle log".to_owned(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(logs[0].correlation_id, first.correlation_id);
    let activity = database
        .activity()
        .list_for_project(
            ActivityActor {
                id: &actor_id,
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
        entry.action == "deployment.create"
            && entry.resource_id.as_deref() == Some(first.id.as_str())
            && entry.correlation_id.as_deref() == Some(first.correlation_id.as_str())
    }));
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
                supply_chain_report: None,
                variables_ciphertext: "different-ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Existing(repeated) = repeated else {
        panic!("same idempotency key must return existing deployment");
    };
    assert_eq!(repeated.id, first.id);
    assert_eq!(repeated.correlation_id, first.correlation_id);
    assert_eq!(repeated.supply_chain_report, Some(report.clone()));
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
                supply_chain_report: None,
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
    assert_ne!(rollback.correlation_id, first.correlation_id);
    assert_eq!(rollback.supply_chain_report, Some(report));
}

#[tokio::test]
async fn production_deployment_requires_owner_approval_before_worker_claim() {
    let database = database().await;
    let owner_id = user_id(&database, "approval-owner").await;
    let editor_id = user_id(&database, "approval-editor").await;
    let project = database
        .projects()
        .create(&owner_id, ProjectInput::new("Approval flow").unwrap())
        .await
        .unwrap();
    database
        .projects()
        .add_member(project.id.as_str(), &editor_id, ProjectMemberRole::Editor)
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
            None,
        )
        .await
        .unwrap();
    let ServiceMutationOutcome::Created(service) = service else {
        panic!("service must be created");
    };
    let owner = crate::DeploymentActor {
        id: &owner_id,
        is_admin: false,
    };
    let created = database
        .deployments()
        .create(
            owner,
            service.id.as_str(),
            crate::NewDeployment {
                idempotency_key: "approval-1".to_owned(),
                requested_by_user_id: owner_id.clone(),
                spec: service.spec.clone(),
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                supply_chain_report: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(created) = created else {
        panic!("deployment must be created");
    };
    assert!(created.approval.is_pending());
    assert!(database.deployments().claim_next().await.unwrap().is_none());

    let editor = crate::DeploymentActor {
        id: &editor_id,
        is_admin: false,
    };
    assert!(matches!(
        database
            .deployments()
            .approve(editor, created.id.as_str())
            .await
            .unwrap(),
        crate::DeploymentApprovalOutcome::Forbidden
    ));

    let approved = database
        .deployments()
        .approve(owner, created.id.as_str())
        .await
        .unwrap();
    let crate::DeploymentApprovalOutcome::Approved(approved) = approved else {
        panic!("owner approval must be recorded");
    };
    assert_eq!(
        approved.approval.status,
        ignitify_domain::ProductionApprovalStatus::Approved
    );
    assert_eq!(
        approved.approval.approved_by_user_id.as_deref(),
        Some(owner_id.as_str())
    );
    assert!(database.deployments().claim_next().await.unwrap().is_some());
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
            None,
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
                supply_chain_report: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let crate::DeploymentApprovalOutcome::Approved(_) = database
        .deployments()
        .approve(actor, deployment.id.as_str())
        .await
        .unwrap()
    else {
        panic!("deployment must be approved before retry testing");
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
    let operations = database.operations().summary().await.unwrap();
    assert_eq!(operations.deployments.queued_count, 1);
    assert_eq!(operations.deployments.retry_count, 1);
    assert_eq!(operations.deployments.failed_retry_count, 0);

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
async fn deployment_retry_exhaustion_persists_failure_and_clears_schedule() {
    let database = database().await;
    let actor_id = user_id(&database, "retry-exhaustion-owner").await;
    let project = database
        .projects()
        .create(&actor_id, ProjectInput::new("Retry exhaustion").unwrap())
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
            None,
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
                idempotency_key: "retry-exhaustion".to_owned(),
                requested_by_user_id: actor_id.clone(),
                spec: service.spec,
                source_config: None,
                deployment_destination_id: None,
                source_revision: None,
                supply_chain_report: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let crate::DeploymentApprovalOutcome::Approved(_) = database
        .deployments()
        .approve(actor, deployment.id.as_str())
        .await
        .unwrap()
    else {
        panic!("deployment must be approved before retry testing");
    };

    let claimed = database.deployments().claim_next().await.unwrap().unwrap();
    let retry = database
        .deployments()
        .schedule_retry(claimed.id.as_str(), "runtime did not start", 1)
        .await
        .unwrap();
    assert!(matches!(retry, crate::RetrySchedule::Exhausted));

    let failed = database
        .deployments()
        .get(actor, deployment.id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.state, ignitify_domain::DeploymentState::Failed);
    assert_eq!(failed.attempt_count, 1);
    assert_eq!(failed.retry_after, None);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("runtime did not start after 1 attempts")
    );
    assert!(failed.finished_at.is_some());
    let events = database
        .deployments()
        .events(deployment.id.as_str())
        .await
        .unwrap();
    assert!(events.iter().any(|event| event.kind == "deployment.failed"));
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
        auto_deploy: false,
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
            None,
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
                supply_chain_report: None,
                variables_ciphertext: "ciphertext".to_owned(),
            },
        )
        .await
        .unwrap();
    let crate::CreateDeploymentOutcome::Created(deployment) = deployment else {
        panic!("deployment must be created");
    };
    let crate::DeploymentApprovalOutcome::Approved(_) = database
        .deployments()
        .approve(
            crate::DeploymentActor {
                id: &actor_id,
                is_admin: false,
            },
            deployment.id.as_str(),
        )
        .await
        .unwrap()
    else {
        panic!("deployment must be approved before source resolution testing");
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
            None,
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
                supply_chain_report: None,
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
            None,
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
        .create(
            owner,
            project.id.as_str(),
            input.configuration,
            variables,
            None,
        )
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
            None,
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
            None,
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
            None,
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
