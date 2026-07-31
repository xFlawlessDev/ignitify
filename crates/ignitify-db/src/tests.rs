use chrono::Utc;
use ignitify_domain::ProjectInput;
use uuid::Uuid;

use crate::{Database, DatabaseConfig, ProjectActor, ProjectUpdateOutcome};

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
