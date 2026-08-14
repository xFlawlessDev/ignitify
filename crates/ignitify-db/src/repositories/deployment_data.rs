use sqlx::FromRow;

use super::{
    DatabaseError, DeploymentId, DeploymentRecord, DeploymentState, DeploymentVariableRecord,
    Result, ServiceId, ServiceSourceConfig, ServiceSpec,
};

pub(super) fn decode_spec(value: &str) -> Result<ServiceSpec> {
    let spec: ServiceSpec = serde_json::from_str(value)
        .map_err(|error| DatabaseError::InvalidServiceSpec(error.to_string()))?;
    spec.validate()
        .map_err(|error| DatabaseError::InvalidServiceSpec(error.to_string()))?;
    Ok(spec)
}

pub(super) fn decode_source_config(value: Option<String>) -> Result<Option<ServiceSourceConfig>> {
    value
        .map(|json| {
            let config = serde_json::from_str::<ServiceSourceConfig>(&json)
                .map_err(|error| DatabaseError::InvalidServiceSourceConfig(error.to_string()))?;
            config
                .validate()
                .map_err(|error| DatabaseError::InvalidServiceSourceConfig(error.to_string()))?;
            Ok(config)
        })
        .transpose()
}

pub(super) fn deployment_from_row(row: DeploymentRow) -> Result<DeploymentRecord> {
    Ok(DeploymentRecord {
        id: DeploymentId::new(row.id)
            .map_err(|_| sqlx::Error::Protocol("stored deployment id is invalid".into()))?,
        correlation_id: row.correlation_id,
        service_id: parse_service_id(row.service_id)?,
        generation: row.generation,
        idempotency_key: row.idempotency_key,
        requested_by_user_id: row.requested_by_user_id,
        spec: row
            .runtime_spec_json
            .as_deref()
            .map(decode_spec)
            .transpose()?
            .unwrap_or(decode_spec(&row.spec_json)?),
        source_config: decode_source_config(row.source_config_json)?,
        deployment_destination_id: row.deployment_destination_id,
        source_revision: row.source_revision,
        local_image_id: row.local_image_id,
        variables_ciphertext: row.variables_ciphertext,
        runtime_ref: row.runtime_ref,
        state: DeploymentState::try_from(row.status.as_str())
            .map_err(|_| DatabaseError::InvalidDeploymentState(row.status))?,
        failure_reason: row.failure_reason,
        attempt_count: row.attempt_count,
        retry_after: row.retry_after,
        cancel_requested_at: row.cancel_requested_at,
        created_at: row.created_at,
        started_at: row.started_at,
        finished_at: row.finished_at,
    })
}

pub(super) fn parse_service_id(value: String) -> Result<ServiceId> {
    ServiceId::new(value)
        .map_err(|_| sqlx::Error::Protocol("stored service id is invalid".into()).into())
}

#[derive(Debug, FromRow)]
pub(super) struct ServiceRow {
    pub(super) id: String,
    pub(super) project_id: String,
    pub(super) desired_generation: i64,
    pub(super) desired_spec_json: String,
    pub(super) source_config_json: Option<String>,
    pub(super) deployment_destination_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct VariableRow {
    pub(super) key: String,
    pub(super) ciphertext: String,
}

impl VariableRow {
    pub(super) fn into_record(self) -> DeploymentVariableRecord {
        DeploymentVariableRecord {
            key: self.key,
            ciphertext: self.ciphertext,
        }
    }
}

#[derive(Debug, FromRow)]
pub(super) struct ProjectVariableRow {
    pub(super) key: String,
    pub(super) ciphertext: String,
}

impl ProjectVariableRow {
    pub(super) fn into_record(self) -> DeploymentVariableRecord {
        DeploymentVariableRecord {
            key: self.key,
            ciphertext: self.ciphertext,
        }
    }
}

#[derive(Debug, FromRow)]
pub(super) struct DeploymentRow {
    pub(super) id: String,
    pub(super) correlation_id: String,
    pub(super) service_id: String,
    pub(super) generation: i64,
    pub(super) idempotency_key: String,
    pub(super) requested_by_user_id: String,
    pub(super) spec_json: String,
    pub(super) runtime_spec_json: Option<String>,
    pub(super) source_config_json: Option<String>,
    pub(super) deployment_destination_id: Option<String>,
    pub(super) source_revision: Option<String>,
    pub(super) local_image_id: Option<String>,
    pub(super) variables_ciphertext: String,
    pub(super) runtime_ref: Option<String>,
    pub(super) status: String,
    pub(super) failure_reason: Option<String>,
    pub(super) attempt_count: i64,
    pub(super) retry_after: Option<String>,
    pub(super) cancel_requested_at: Option<String>,
    pub(super) created_at: String,
    pub(super) started_at: Option<String>,
    pub(super) finished_at: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct DeploymentWithProjectRow {
    pub(super) id: String,
    pub(super) correlation_id: String,
    pub(super) service_id: String,
    pub(super) generation: i64,
    pub(super) idempotency_key: String,
    pub(super) requested_by_user_id: String,
    pub(super) spec_json: String,
    pub(super) runtime_spec_json: Option<String>,
    pub(super) source_config_json: Option<String>,
    pub(super) deployment_destination_id: Option<String>,
    pub(super) source_revision: Option<String>,
    pub(super) local_image_id: Option<String>,
    pub(super) variables_ciphertext: String,
    pub(super) runtime_ref: Option<String>,
    pub(super) status: String,
    pub(super) failure_reason: Option<String>,
    pub(super) attempt_count: i64,
    pub(super) retry_after: Option<String>,
    pub(super) cancel_requested_at: Option<String>,
    pub(super) created_at: String,
    pub(super) started_at: Option<String>,
    pub(super) finished_at: Option<String>,
    pub(super) project_id: String,
}

impl From<DeploymentWithProjectRow> for DeploymentRow {
    fn from(row: DeploymentWithProjectRow) -> Self {
        Self {
            id: row.id,
            correlation_id: row.correlation_id,
            service_id: row.service_id,
            generation: row.generation,
            idempotency_key: row.idempotency_key,
            requested_by_user_id: row.requested_by_user_id,
            spec_json: row.spec_json,
            runtime_spec_json: row.runtime_spec_json,
            source_config_json: row.source_config_json,
            deployment_destination_id: row.deployment_destination_id,
            source_revision: row.source_revision,
            local_image_id: row.local_image_id,
            variables_ciphertext: row.variables_ciphertext,
            runtime_ref: row.runtime_ref,
            status: row.status,
            failure_reason: row.failure_reason,
            attempt_count: row.attempt_count,
            retry_after: row.retry_after,
            cancel_requested_at: row.cancel_requested_at,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}
