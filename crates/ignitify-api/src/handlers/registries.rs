use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_db::{NewRegistry, RegistryActor, RegistryRecord};
use serde::{Deserialize, Serialize};
use url::Url;
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistryRequest {
    name: String,
    endpoint: String,
    username: Option<String>,
    credential: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveRegistryRequest {
    confirm_name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryResponse {
    id: String,
    name: String,
    endpoint: String,
    username: Option<String>,
    credential_configured: bool,
    created_at: String,
    updated_at: String,
}

impl From<RegistryRecord> for RegistryResponse {
    fn from(record: RegistryRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            endpoint: record.endpoint,
            username: record.username,
            credential_configured: record.credential_ciphertext.is_some(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RegistryResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let registries = state
        .database
        .registries()
        .list(registry_actor(&actor))
        .await?
        .ok_or(ApiError::Forbidden)?
        .into_iter()
        .map(RegistryResponse::from)
        .collect();
    Ok(Json(registries))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegistryRequest>,
) -> Result<(StatusCode, Json<RegistryResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if !actor.has_admin_access() {
        return Err(ApiError::Forbidden);
    }
    let registry = input(&state, request)?;
    let record = state
        .database
        .registries()
        .create(registry_actor(&actor), registry)
        .await?
        .ok_or(ApiError::Forbidden)?;
    Ok((StatusCode::CREATED, Json(record.into())))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(registry_id): Path<String>,
    Json(request): Json<RemoveRegistryRequest>,
) -> Result<Json<RegistryResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if !actor.has_admin_access() {
        return Err(ApiError::Forbidden);
    }
    let record = state
        .database
        .registries()
        .delete(registry_actor(&actor), &registry_id, &request.confirm_name)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(record.into()))
}

fn registry_actor(actor: &ignitify_auth::AuthenticatedUser) -> RegistryActor<'_> {
    RegistryActor {
        is_admin: actor.has_admin_access(),
        user_id: &actor.id,
    }
}

fn input(state: &AppState, request: RegistryRequest) -> Result<NewRegistry, ApiError> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > 100 || name.chars().any(char::is_control) {
        return Err(ApiError::BadRequest("invalid registry name"));
    }
    let endpoint = Url::parse(&request.endpoint)
        .map_err(|_| ApiError::BadRequest("invalid registry endpoint"))?;
    if endpoint.scheme() != "https"
        || endpoint.host_str().is_none()
        || endpoint.username() != ""
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(ApiError::BadRequest("invalid registry endpoint"));
    }
    let endpoint = endpoint.to_string().trim_end_matches('/').to_owned();
    let username = request
        .username
        .map(|username| username.trim().to_owned())
        .filter(|username| !username.is_empty());
    let credential = request
        .credential
        .map(Zeroizing::new)
        .filter(|credential| !credential.is_empty());
    if credential
        .as_ref()
        .is_some_and(|credential| credential.len() > 16 * 1024 || credential.contains('\0'))
    {
        return Err(ApiError::BadRequest("invalid registry credential"));
    }
    let credential_ciphertext = credential
        .map(|credential| state.control.worker_cipher().encrypt(credential.as_bytes()))
        .transpose()?;
    if username.is_some() != credential_ciphertext.is_some() {
        return Err(ApiError::BadRequest(
            "registry username and credential must be provided together",
        ));
    }
    Ok(NewRegistry {
        name: name.to_owned(),
        endpoint,
        username,
        credential_ciphertext,
    })
}

#[cfg(test)]
mod tests {
    use super::RegistryRequest;

    #[test]
    fn rejects_registry_endpoint_with_embedded_credentials() {
        let request = RegistryRequest {
            name: "private".to_owned(),
            endpoint: "https://user:pass@registry.example.com".to_owned(),
            username: None,
            credential: None,
        };
        assert!(input_stub(request).is_err());
    }

    fn input_stub(request: RegistryRequest) -> Result<(), ()> {
        let endpoint = url::Url::parse(&request.endpoint).map_err(|_| ())?;
        if endpoint.username() != "" || endpoint.password().is_some() {
            return Err(());
        }
        Ok(())
    }
}
