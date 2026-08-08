use chrono::Utc;
use ignitify_domain::{
    ApplicationBuilder, DomainName, ProjectInput, ProjectMemberRole, ServiceInput,
    ServiceSourceConfig, ServiceVariableInput,
};
use uuid::Uuid;

use crate::{
    ActivityActor, Database, DatabaseConfig, DomainActor, NewProvider, NewServiceVariable,
    ProjectActor, ProjectUpdateOutcome, ProviderAuthMode, ProviderKind, ServiceActor,
    ServiceMutationOutcome,
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
    assert!(
        database
            .deployments()
            .record_source_revision(
                claimed.id.as_str(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .await
            .unwrap()
    );
    database
        .deployments()
        .record_source_build(
            claimed.id.as_str(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .await
        .unwrap();
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
        )
        .await
        .unwrap();
    let crate::DomainMutationOutcome::Created(domain) = created else {
        panic!("domain must be created");
    };
    let duplicate = database
        .domains()
        .create(
            owner,
            service.id.as_str(),
            DomainName::new("app.example.com").unwrap(),
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
