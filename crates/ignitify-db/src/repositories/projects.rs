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
pub struct NewProjectVariable {
    pub key: String,
    pub is_secret: bool,
    pub ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct ProjectVariableRecord {
    pub key: String,
    pub is_secret: bool,
    pub ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct AuthorizedProjectVariables {
    pub role: ProjectMemberRole,
    pub variables: Vec<ProjectVariableRecord>,
}

#[derive(Debug, Clone)]
pub enum ProjectVariablesMutationOutcome {
    Updated(AuthorizedProjectVariables),
    Missing,
    Forbidden,
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

    pub async fn variables(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
    ) -> Result<Option<AuthorizedProjectVariables>> {
        let Some(project) = self.get(actor.clone(), project_id).await? else {
            return Ok(None);
        };
        let rows = sqlx::query_as::<_, ProjectVariableRow>(
            "SELECT key, is_secret, ciphertext
             FROM project_variables
             WHERE project_id = ?
             ORDER BY key",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(AuthorizedProjectVariables {
            role: project.role,
            variables: rows
                .into_iter()
                .map(ProjectVariableRow::into_record)
                .collect(),
        }))
    }

    pub async fn replace_variables(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
        variables: Vec<NewProjectVariable>,
    ) -> Result<ProjectVariablesMutationOutcome> {
        let Some(project) = self.get(actor.clone(), project_id).await? else {
            return Ok(ProjectVariablesMutationOutcome::Missing);
        };
        if !actor.is_admin && !project.role.can_manage_services() {
            return Ok(ProjectVariablesMutationOutcome::Forbidden);
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM project_variables WHERE project_id = ?")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for variable in variables {
            sqlx::query(
                "INSERT INTO project_variables
                 (id, project_id, key, is_secret, ciphertext, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(project_id)
            .bind(variable.key)
            .bind(variable.is_secret)
            .bind(variable.ciphertext)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("UPDATE projects SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        insert_environment_audit(&mut tx, actor.id, project_id, &now).await?;
        tx.commit().await?;

        self.variables(
            ProjectActor {
                id: actor.id,
                is_admin: actor.is_admin,
            },
            project_id,
        )
        .await?
        .map(ProjectVariablesMutationOutcome::Updated)
        .ok_or_else(|| sqlx::Error::RowNotFound.into())
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

    pub async fn remove(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
        confirm_name: &str,
    ) -> Result<ProjectRemoveOutcome> {
        let Some(project) = self.get(actor.clone(), project_id).await? else {
            return Ok(ProjectRemoveOutcome::Missing);
        };
        if !actor.is_admin && !project.role.can_update_project() {
            return Ok(ProjectRemoveOutcome::Forbidden);
        }
        if project.name != confirm_name {
            return Err(DatabaseError::ProjectConfirmationMismatch);
        }

        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(
               SELECT 1
               FROM services s
               JOIN environments e ON e.id = s.environment_id
               JOIN deployments d ON d.service_id = s.id
               WHERE e.project_id = ?
                 AND d.status IN ('queued', 'preparing', 'running', 'healthy', 'stopping')
             )",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if active {
            return Err(DatabaseError::ProjectHasActiveDeployment);
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
             VALUES (?, ?, 'project.remove', 'project', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(actor.id)
        .bind(project_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM projects WHERE id = ? AND name = ?")
            .bind(project_id)
            .bind(confirm_name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(ProjectRemoveOutcome::Removed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectUpdateOutcome {
    Updated(ProjectSummary),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRemoveOutcome {
    Removed,
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

#[derive(Debug, FromRow)]
struct ProjectVariableRow {
    key: String,
    is_secret: bool,
    ciphertext: String,
}

impl ProjectVariableRow {
    fn into_record(self) -> ProjectVariableRecord {
        ProjectVariableRecord {
            key: self.key,
            is_secret: self.is_secret,
            ciphertext: self.ciphertext,
        }
    }
}

async fn insert_environment_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    project_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
         VALUES (?, ?, 'project.environment.update', 'project', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(project_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
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
