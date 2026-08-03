use chrono::Utc;
use ignitify_domain::{
    EnvironmentId, EnvironmentSummary, ProjectId, ProjectInput, ProjectMemberRole, ProjectSummary,
    UserId,
};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone)]
pub struct ProjectActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectsRepository {
    pool: SqlitePool,
}

impl ProjectsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, actor: ProjectActor<'_>) -> Result<Vec<ProjectSummary>> {
        let rows = sqlx::query_as::<_, ProjectSummaryRow>(
            "SELECT p.id, p.name, p.owner_id, p.created_at, p.updated_at, \
                    COALESCE(pm.role, 'owner') AS role, e.id AS environment_id, e.name AS environment_name, e.is_default \
             FROM projects p \
             JOIN environments e ON e.project_id = p.id AND e.is_default = 1 \
             LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ? \
             WHERE ? OR pm.user_id IS NOT NULL \
             ORDER BY p.updated_at DESC",
        )
        .bind(actor.id)
        .bind(actor.is_admin)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(ProjectSummaryRow::into_summary)
            .collect()
    }

    pub async fn create(&self, actor_id: &str, input: ProjectInput) -> Result<ProjectSummary> {
        let project_id = Uuid::new_v4().to_string();
        let environment_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        let insert = sqlx::query(
            "INSERT INTO projects (id, name, owner_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&project_id)
        .bind(&input.name)
        .bind(actor_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await;
        if let Err(error) = insert {
            return match error {
                sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                    Err(DatabaseError::ProjectNameConflict)
                }
                error => Err(error.into()),
            };
        }

        sqlx::query(
            "INSERT INTO project_members (project_id, user_id, role, created_at) VALUES (?, ?, 'owner', ?)",
        )
        .bind(&project_id)
        .bind(actor_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO environments (id, project_id, name, is_default, created_at, updated_at) VALUES (?, ?, 'production', 1, ?, ?)",
        )
        .bind(&environment_id)
        .bind(&project_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
             VALUES (?, ?, 'project.create', 'project', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(actor_id)
        .bind(&project_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(ProjectSummary {
            id: ProjectId::new(project_id)
                .map_err(|_| sqlx::Error::Protocol("generated project id is invalid".into()))?,
            name: input.name,
            owner_id: UserId::new(actor_id)
                .map_err(|_| sqlx::Error::Protocol("actor id is invalid".into()))?,
            role: ProjectMemberRole::Owner,
            created_at: now.clone(),
            updated_at: now,
            default_environment: EnvironmentSummary {
                id: EnvironmentId::new(environment_id).map_err(|_| {
                    sqlx::Error::Protocol("generated environment id is invalid".into())
                })?,
                name: "production".to_owned(),
                is_default: true,
            },
        })
    }

    pub async fn add_member(
        &self,
        project_id: &str,
        user_id: &str,
        role: ProjectMemberRole,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO project_members (project_id, user_id, role, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(project_id)
        .bind(user_id)
        .bind(role.as_str())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
    ) -> Result<Option<ProjectSummary>> {
        let row = sqlx::query_as::<_, ProjectSummaryRow>(
            "SELECT p.id, p.name, p.owner_id, p.created_at, p.updated_at, \
                    COALESCE(pm.role, 'owner') AS role, e.id AS environment_id, e.name AS environment_name, e.is_default \
             FROM projects p \
             JOIN environments e ON e.project_id = p.id AND e.is_default = 1 \
             LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ? \
             WHERE p.id = ? AND (? OR pm.user_id IS NOT NULL)",
        )
        .bind(actor.id)
        .bind(project_id)
        .bind(actor.is_admin)
        .fetch_optional(&self.pool)
        .await?;
        row.map(ProjectSummaryRow::into_summary).transpose()
    }

    pub async fn rename(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
        input: ProjectInput,
    ) -> Result<ProjectUpdateOutcome> {
        let Some(project) = self.get(actor.clone(), project_id).await? else {
            return Ok(ProjectUpdateOutcome::Missing);
        };
        if !actor.is_admin && !project.role.can_update_project() {
            return Ok(ProjectUpdateOutcome::Forbidden);
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let update = sqlx::query("UPDATE projects SET name = ?, updated_at = ? WHERE id = ?")
            .bind(&input.name)
            .bind(&now)
            .bind(project_id)
            .execute(&mut *tx)
            .await;
        if let Err(error) = update {
            return match error {
                sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                    Err(DatabaseError::ProjectNameConflict)
                }
                error => Err(error.into()),
            };
        }
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
             VALUES (?, ?, 'project.update', 'project', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(actor.id)
        .bind(project_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get(actor, project_id)
            .await?
            .map(ProjectUpdateOutcome::Updated)
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectUpdateOutcome {
    Updated(ProjectSummary),
    Missing,
    Forbidden,
}

#[derive(FromRow)]
struct ProjectSummaryRow {
    id: String,
    name: String,
    owner_id: String,
    created_at: String,
    updated_at: String,
    role: String,
    environment_id: String,
    environment_name: String,
    is_default: bool,
}

impl ProjectSummaryRow {
    fn into_summary(self) -> Result<ProjectSummary> {
        let role = ProjectMemberRole::try_from(self.role.as_str())
            .map_err(|_| DatabaseError::InvalidProjectMemberRole(self.role))?;
        Ok(ProjectSummary {
            id: ProjectId::new(self.id)
                .map_err(|_| sqlx::Error::Protocol("stored project id is invalid".into()))?,
            name: self.name,
            owner_id: UserId::new(self.owner_id)
                .map_err(|_| sqlx::Error::Protocol("stored owner id is invalid".into()))?,
            role,
            created_at: self.created_at,
            updated_at: self.updated_at,
            default_environment: EnvironmentSummary {
                id: EnvironmentId::new(self.environment_id).map_err(|_| {
                    sqlx::Error::Protocol("stored environment id is invalid".into())
                })?,
                name: self.environment_name,
                is_default: self.is_default,
            },
        })
    }
}
