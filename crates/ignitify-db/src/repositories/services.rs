use std::collections::HashMap;

use chrono::Utc;
use ignitify_domain::{
    EnvironmentId, ProjectId, ProjectMemberRole, ServiceConfiguration, ServiceId, ServiceKind,
    ServiceSourceConfig, ServiceSpec,
};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone, Copy)]
pub struct ServiceActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct NewServiceVariable {
    pub key: String,
    pub is_secret: bool,
    pub ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct AuthorizedService {
    pub id: ServiceId,
    pub project_id: ProjectId,
    pub environment_id: EnvironmentId,
    pub role: ProjectMemberRole,
    pub name: String,
    pub kind: ServiceKind,
    pub spec: ServiceSpec,
    pub source_config: Option<ServiceSourceConfig>,
    pub deployment_destination_id: Option<String>,
    pub desired_generation: i64,
    pub desired_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub variables: Vec<ServiceVariableRecord>,
}

#[derive(Debug, Clone)]
pub struct ServiceVariableRecord {
    pub key: String,
    pub is_secret: bool,
    pub ciphertext: String,
}

#[derive(Debug, Clone)]
pub enum ServiceMutationOutcome {
    Created(AuthorizedService),
    Updated(AuthorizedService),
    Removed(AuthorizedService),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub struct ServicesRepository {
    pool: SqlitePool,
}

impl ServicesRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
    ) -> Result<Option<Vec<AuthorizedService>>> {
        let Some(role) = self.project_role(actor, project_id).await? else {
            return Ok(None);
        };
        let rows = sqlx::query_as::<_, ServiceRow>(
            "SELECT s.id, e.project_id, s.environment_id, s.name, s.kind, s.desired_spec_json,
                    s.source_config_json, s.deployment_destination_id,
                    s.desired_generation, s.desired_state, s.created_at, s.updated_at
             FROM services s
             JOIN environments e ON e.id = s.environment_id
             WHERE e.project_id = ?
             ORDER BY s.updated_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let mut variables_by_service = self.variables_for_project(project_id).await?;
        let mut services = Vec::with_capacity(rows.len());
        for row in rows {
            let variables = variables_by_service.remove(&row.id).unwrap_or_default();
            services.push(Self::read_service(row, role, variables)?);
        }
        Ok(Some(services))
    }

    pub async fn get(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
    ) -> Result<Option<AuthorizedService>> {
        let row = sqlx::query_as::<_, ServiceRow>(
            "SELECT s.id, e.project_id, s.environment_id, s.name, s.kind, s.desired_spec_json,
                    s.source_config_json, s.deployment_destination_id,
                    s.desired_generation, s.desired_state, s.created_at, s.updated_at
             FROM services s
             JOIN environments e ON e.id = s.environment_id
             WHERE s.id = ?",
        )
        .bind(service_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(role) = self.project_role(actor, &row.project_id).await? else {
            return Ok(None);
        };
        let variables = self.variables_for_service(&row.id).await?;
        Ok(Some(Self::read_service(row, role, variables)?))
    }

    pub async fn create(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
        configuration: ServiceConfiguration,
        variables: Vec<NewServiceVariable>,
    ) -> Result<ServiceMutationOutcome> {
        let Some(role) = self.project_role(actor, project_id).await? else {
            return Ok(ServiceMutationOutcome::Missing);
        };
        if !actor.is_admin && !role.can_manage_services() {
            return Ok(ServiceMutationOutcome::Forbidden);
        }
        let environment_id: String = sqlx::query_scalar(
            "SELECT id FROM environments WHERE project_id = ? AND is_default = 1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
        let service_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let spec_json = serde_json::to_string(&configuration.spec)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let source_config_json = configuration
            .source_config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let mut tx = self.pool.begin().await?;
        ensure_destination(&mut tx, configuration.deployment_destination_id.as_deref()).await?;
        let insert = sqlx::query(
            "INSERT INTO services (id, environment_id, name, kind, desired_spec_json, source_config_json, deployment_destination_id, desired_generation, desired_state, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 1, 'stopped', ?, ?)",
        )
        .bind(&service_id)
        .bind(&environment_id)
        .bind(&configuration.name)
        .bind(configuration.spec.kind().as_str())
        .bind(&spec_json)
        .bind(source_config_json)
        .bind(&configuration.deployment_destination_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await;
        if let Err(error) = insert {
            if let sqlx::Error::Database(database_error) = &error
                && database_error.is_unique_violation()
            {
                return Err(DatabaseError::ServiceNameConflict);
            }
            return Err(error.into());
        }
        insert_variables(&mut tx, &service_id, &variables, &now).await?;
        insert_audit(&mut tx, actor.id, "service.create", &service_id, &now).await?;
        tx.commit().await?;
        Ok(ServiceMutationOutcome::Created(
            self.get(actor, &service_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?,
        ))
    }

    pub async fn update(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
        configuration: ServiceConfiguration,
        variables: Vec<NewServiceVariable>,
    ) -> Result<ServiceMutationOutcome> {
        let Some(current) = self.get(actor, service_id).await? else {
            return Ok(ServiceMutationOutcome::Missing);
        };
        if !actor.is_admin && !current.role.can_manage_services() {
            return Ok(ServiceMutationOutcome::Forbidden);
        }
        let spec_json = serde_json::to_string(&configuration.spec)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let source_config_json = configuration
            .source_config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        ensure_destination(&mut tx, configuration.deployment_destination_id.as_deref()).await?;
        let update = sqlx::query(
            "UPDATE services
             SET name = ?, kind = ?, desired_spec_json = ?, source_config_json = ?, deployment_destination_id = ?, desired_generation = desired_generation + 1, updated_at = ?
             WHERE id = ?",
        )
        .bind(&configuration.name)
        .bind(configuration.spec.kind().as_str())
        .bind(&spec_json)
        .bind(source_config_json)
        .bind(&configuration.deployment_destination_id)
        .bind(&now)
        .bind(service_id)
        .execute(&mut *tx)
        .await;
        if let Err(error) = update {
            if let sqlx::Error::Database(database_error) = &error
                && database_error.is_unique_violation()
            {
                return Err(DatabaseError::ServiceNameConflict);
            }
            return Err(error.into());
        }
        sqlx::query("DELETE FROM service_variables WHERE service_id = ?")
            .bind(service_id)
            .execute(&mut *tx)
            .await?;
        insert_variables(&mut tx, service_id, &variables, &now).await?;
        insert_audit(&mut tx, actor.id, "service.update", service_id, &now).await?;
        tx.commit().await?;
        Ok(ServiceMutationOutcome::Updated(
            self.get(actor, service_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?,
        ))
    }

    pub async fn remove(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
        confirm_name: &str,
    ) -> Result<ServiceMutationOutcome> {
        let Some(current) = self.get(actor, service_id).await? else {
            return Ok(ServiceMutationOutcome::Missing);
        };
        if !actor.is_admin && !current.role.can_manage_services() {
            return Ok(ServiceMutationOutcome::Forbidden);
        }
        if current.name != confirm_name {
            return Err(DatabaseError::ServiceConfirmationMismatch);
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM services
             WHERE id = ? AND name = ?
               AND NOT EXISTS (
                 SELECT 1 FROM deployments
                 WHERE service_id = ?
                   AND status IN ('queued', 'preparing', 'running', 'healthy', 'stopping')
               )",
        )
        .bind(service_id)
        .bind(confirm_name)
        .bind(service_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if deleted == 0 {
            tx.rollback().await?;
            let active: bool = sqlx::query_scalar(
                "SELECT EXISTS(
                   SELECT 1 FROM deployments
                   WHERE service_id = ?
                     AND status IN ('queued', 'preparing', 'running', 'healthy', 'stopping')
                 )",
            )
            .bind(service_id)
            .fetch_one(&self.pool)
            .await?;
            return if active {
                Err(DatabaseError::ServiceHasActiveDeployment)
            } else {
                Err(DatabaseError::ServiceConfirmationMismatch)
            };
        }
        insert_audit(&mut tx, actor.id, "service.remove", service_id, &now).await?;
        tx.commit().await?;
        Ok(ServiceMutationOutcome::Removed(current))
    }

    async fn project_role(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
    ) -> Result<Option<ProjectMemberRole>> {
        if actor.is_admin {
            let exists: Option<String> = sqlx::query_scalar("SELECT id FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await?;
            return Ok(exists.map(|_| ProjectMemberRole::Owner));
        }
        let role: Option<String> = sqlx::query_scalar(
            "SELECT pm.role
             FROM project_members pm
             WHERE pm.project_id = ? AND pm.user_id = ?",
        )
        .bind(project_id)
        .bind(actor.id)
        .fetch_optional(&self.pool)
        .await?;
        role.map(|role| {
            role.as_str()
                .try_into()
                .map_err(|_| DatabaseError::InvalidProjectMemberRole(role))
        })
        .transpose()
    }

    async fn variables_for_project(
        &self,
        project_id: &str,
    ) -> Result<HashMap<String, Vec<ServiceVariableRecord>>> {
        let rows = sqlx::query_as::<_, VariableRow>(
            "SELECT sv.service_id, sv.key, sv.is_secret, sv.ciphertext
             FROM service_variables sv
             JOIN services s ON s.id = sv.service_id
             JOIN environments e ON e.id = s.environment_id
             WHERE e.project_id = ?
             ORDER BY sv.service_id, sv.key",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(variable_map(rows))
    }

    async fn variables_for_service(&self, service_id: &str) -> Result<Vec<ServiceVariableRecord>> {
        let rows = sqlx::query_as::<_, VariableRow>(
            "SELECT service_id, key, is_secret, ciphertext
             FROM service_variables WHERE service_id = ? ORDER BY key",
        )
        .bind(service_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(variable_map(rows).remove(service_id).unwrap_or_default())
    }

    fn read_service(
        row: ServiceRow,
        role: ProjectMemberRole,
        variables: Vec<ServiceVariableRecord>,
    ) -> Result<AuthorizedService> {
        let kind = row
            .kind
            .as_str()
            .try_into()
            .map_err(|_| DatabaseError::InvalidServiceKind(row.kind.clone()))?;
        let spec: ServiceSpec = serde_json::from_str(&row.desired_spec_json)
            .map_err(|error| DatabaseError::InvalidServiceSpec(error.to_string()))?;
        spec.validate()
            .map_err(|error| DatabaseError::InvalidServiceSpec(error.to_string()))?;
        if spec.kind() != kind {
            return Err(DatabaseError::InvalidServiceSpec(
                "service kind does not match desired specification".to_owned(),
            ));
        }
        let source_config = row
            .source_config_json
            .map(|json| {
                serde_json::from_str::<ServiceSourceConfig>(&json)
                    .map_err(|error| DatabaseError::InvalidServiceSourceConfig(error.to_string()))
            })
            .transpose()?;
        Ok(AuthorizedService {
            id: ServiceId::new(row.id)
                .map_err(|_| sqlx::Error::Protocol("stored service id is invalid".into()))?,
            project_id: ProjectId::new(row.project_id)
                .map_err(|_| sqlx::Error::Protocol("stored project id is invalid".into()))?,
            environment_id: EnvironmentId::new(row.environment_id)
                .map_err(|_| sqlx::Error::Protocol("stored environment id is invalid".into()))?,
            role,
            name: row.name,
            kind,
            spec,
            source_config,
            deployment_destination_id: row.deployment_destination_id,
            desired_generation: row.desired_generation,
            desired_state: row.desired_state,
            created_at: row.created_at,
            updated_at: row.updated_at,
            variables,
        })
    }
}

fn variable_map(rows: Vec<VariableRow>) -> HashMap<String, Vec<ServiceVariableRecord>> {
    let mut variables = HashMap::<String, Vec<ServiceVariableRecord>>::new();
    for row in rows {
        variables
            .entry(row.service_id)
            .or_default()
            .push(ServiceVariableRecord {
                key: row.key,
                is_secret: row.is_secret,
                ciphertext: row.ciphertext,
            });
    }
    variables
}

async fn insert_variables(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    service_id: &str,
    variables: &[NewServiceVariable],
    now: &str,
) -> Result<()> {
    for variable in variables {
        sqlx::query(
            "INSERT INTO service_variables (id, service_id, key, is_secret, ciphertext, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(service_id)
        .bind(&variable.key)
        .bind(variable.is_secret)
        .bind(&variable.ciphertext)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn ensure_destination(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    destination_id: Option<&str>,
) -> Result<()> {
    let Some(destination_id) = destination_id else {
        return Ok(());
    };
    let exists: i64 =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM remote_servers WHERE id = ?)")
            .bind(destination_id)
            .fetch_one(&mut **tx)
            .await?;
    if exists == 0 {
        return Err(DatabaseError::RemoteServerNotFound);
    }
    Ok(())
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    action: &str,
    resource_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
         VALUES (?, ?, ?, 'service', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(action)
    .bind(resource_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
struct ServiceRow {
    id: String,
    project_id: String,
    environment_id: String,
    name: String,
    kind: String,
    desired_spec_json: String,
    source_config_json: Option<String>,
    deployment_destination_id: Option<String>,
    desired_generation: i64,
    desired_state: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct VariableRow {
    service_id: String,
    key: String,
    is_secret: bool,
    ciphertext: String,
}
