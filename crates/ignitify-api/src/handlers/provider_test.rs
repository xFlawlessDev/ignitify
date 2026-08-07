use std::time::Duration;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
};
use chrono::Utc;
use ignitify_auth::AuthenticatedUser;
use ignitify_db::{ProviderAuthMode, ProviderKind, ProviderRecord};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::{Client, RequestBuilder, header::HeaderMap as ResponseHeaders};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub(crate) struct ProviderConnectionResponse {
    repository_count: Option<u64>,
    checked_at: String,
}

#[derive(Debug, Deserialize)]
struct StoredCredentials {
    token: Option<String>,
    private_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubInstallation {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct GithubAccessToken {
    token: String,
}

#[derive(Debug, Deserialize)]
struct GithubInstallationRepositories {
    total_count: Option<u64>,
    repositories: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
struct GithubAppClaims {
    iat: i64,
    exp: i64,
    iss: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProviderRepositoryResponse {
    pub name: String,
    pub path: String,
    pub default_branch: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProviderBranchResponse {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProviderRepositoryQuery {
    repository: String,
}

pub(crate) async fn repositories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Json<Vec<ProviderRepositoryResponse>>, ApiError> {
    require_actor(&state, &headers).await?;
    let provider = state
        .database
        .providers()
        .get(&provider_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let credentials = decrypt_credentials(&state, &provider)?;
    let client = provider_client()?;
    Ok(Json(
        list_provider_repositories(&client, &provider, &credentials).await?,
    ))
}

pub(crate) async fn branches(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Query(query): Query<ProviderRepositoryQuery>,
) -> Result<Json<Vec<ProviderBranchResponse>>, ApiError> {
    require_actor(&state, &headers).await?;
    let repository = query.repository.trim();
    if repository.is_empty() || repository.len() > 1024 || repository.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest("repository is invalid"));
    }
    let provider = state
        .database
        .providers()
        .get(&provider_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let credentials = decrypt_credentials(&state, &provider)?;
    let client = provider_client()?;
    Ok(Json(
        list_provider_branches(&client, &provider, &credentials, repository).await?,
    ))
}

fn provider_client() -> Result<Client, ApiError> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(ApiError::ProviderRemote)
}

async fn list_provider_repositories(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<Vec<ProviderRepositoryResponse>, ApiError> {
    let values = match (provider.kind, provider.auth_mode) {
        (ProviderKind::Github, ProviderAuthMode::GithubApp) => {
            return list_github_app_repositories(client, provider, credentials).await;
        }
        (ProviderKind::Github, ProviderAuthMode::Token | ProviderAuthMode::OAuth) => {
            let token = required_token(credentials)?;
            authorized(
                client.get("https://api.github.com/user/repos?per_page=100&affiliation=owner,collaborator,organization_member"),
                ProviderKind::Github,
                token,
            )
                .send()
                .await?
                .error_for_status()?
                .json::<Vec<Value>>()
                .await?
        }
        (ProviderKind::Gitlab, ProviderAuthMode::Token | ProviderAuthMode::OAuth) => {
            let token = required_token(credentials)?;
            authorized(
                client.get(provider_endpoint(
                    &provider.base_url,
                    "/api/v4/projects?membership=true&per_page=100&simple=false",
                )?),
                ProviderKind::Gitlab,
                token,
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Value>>()
            .await?
        }
        (ProviderKind::Gitea, ProviderAuthMode::Token | ProviderAuthMode::OAuth) => {
            let token = required_token(credentials)?;
            authorized(
                client.get(provider_endpoint(
                    &provider.base_url,
                    "/api/v1/user/repos?limit=100",
                )?),
                ProviderKind::Gitea,
                token,
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Value>>()
            .await?
        }
        (ProviderKind::Git, _) => {
            return Err(ApiError::BadRequest(
                "this provider does not support repository discovery",
            ));
        }
        (_, ProviderAuthMode::GithubApp) => {
            return Err(ApiError::BadRequest(
                "GitHub App mode is only available for GitHub",
            ));
        }
    };
    Ok(repository_responses(values, provider.kind))
}

async fn list_provider_branches(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
    repository: &str,
) -> Result<Vec<ProviderBranchResponse>, ApiError> {
    let values = match (provider.kind, provider.auth_mode) {
        (ProviderKind::Github, ProviderAuthMode::Token | ProviderAuthMode::OAuth) => {
            let (owner, name) = repository_parts(repository)?;
            let token = required_token(credentials)?;
            authorized(
                client.get(format!(
                    "https://api.github.com/repos/{}/{}/branches?per_page=100",
                    url_segment(owner),
                    url_segment(name)
                )),
                ProviderKind::Github,
                token,
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Value>>()
            .await?
        }
        (ProviderKind::Gitlab, ProviderAuthMode::Token | ProviderAuthMode::OAuth) => {
            let token = required_token(credentials)?;
            let encoded =
                url::form_urlencoded::byte_serialize(repository.as_bytes()).collect::<String>();
            authorized(
                client.get(provider_endpoint(
                    &provider.base_url,
                    &format!("/api/v4/projects/{encoded}/repository/branches?per_page=100"),
                )?),
                ProviderKind::Gitlab,
                token,
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Value>>()
            .await?
        }
        (ProviderKind::Gitea, ProviderAuthMode::Token | ProviderAuthMode::OAuth) => {
            let (owner, name) = repository_parts(repository)?;
            let token = required_token(credentials)?;
            authorized(
                client.get(provider_endpoint(
                    &provider.base_url,
                    &format!(
                        "/api/v1/repos/{}/{}/branches?limit=100",
                        url_segment(owner),
                        url_segment(name)
                    ),
                )?),
                ProviderKind::Gitea,
                token,
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Value>>()
            .await?
        }
        (ProviderKind::Github, ProviderAuthMode::GithubApp) => {
            let (owner, name) = repository_parts(repository)?;
            return github_app_branches(client, provider, credentials, owner, name).await;
        }
        (ProviderKind::Git, _) => {
            return Err(ApiError::BadRequest(
                "this provider does not support branch discovery",
            ));
        }
        (_, ProviderAuthMode::GithubApp) => {
            return Err(ApiError::BadRequest(
                "GitHub App mode is only available for GitHub",
            ));
        }
    };
    Ok(branch_responses(values))
}

fn repository_responses(values: Vec<Value>, kind: ProviderKind) -> Vec<ProviderRepositoryResponse> {
    let (path_key, name_key) = match kind {
        ProviderKind::Gitlab => ("path_with_namespace", "name"),
        _ => ("full_name", "name"),
    };
    let mut repositories = values
        .into_iter()
        .filter_map(|value| {
            let path = value.get(path_key).and_then(Value::as_str)?;
            let name = value.get(name_key).and_then(Value::as_str).unwrap_or(path);
            Some(ProviderRepositoryResponse {
                name: name.to_owned(),
                path: path.to_owned(),
                default_branch: value
                    .get("default_branch")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect::<Vec<_>>();
    repositories.sort_by(|left, right| left.path.cmp(&right.path));
    repositories.dedup_by(|left, right| left.path == right.path);
    repositories
}

fn branch_responses(values: Vec<Value>) -> Vec<ProviderBranchResponse> {
    values
        .into_iter()
        .filter_map(|value| {
            value
                .get("name")
                .and_then(Value::as_str)
                .map(|name| ProviderBranchResponse {
                    name: name.to_owned(),
                })
        })
        .collect()
}

fn repository_parts(repository: &str) -> Result<(&str, &str), ApiError> {
    let Some((owner, name)) = repository.split_once('/') else {
        return Err(ApiError::BadRequest(
            "repository must use owner/name format",
        ));
    };
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return Err(ApiError::BadRequest(
            "repository must use owner/name format",
        ));
    }
    Ok((owner, name))
}

fn url_segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

async fn list_github_app_repositories(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<Vec<ProviderRepositoryResponse>, ApiError> {
    let mut values = Vec::new();
    for token in github_app_installation_tokens(client, provider, credentials).await? {
        let repositories = github_request(
            client.get("https://api.github.com/installation/repositories?per_page=100"),
            &token,
        )
        .send()
        .await?
        .error_for_status()?
        .json::<GithubInstallationRepositories>()
        .await?;
        values.extend(repositories.repositories.unwrap_or_default());
    }
    Ok(repository_responses(values, ProviderKind::Github))
}

async fn github_app_branches(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
    owner: &str,
    name: &str,
) -> Result<Vec<ProviderBranchResponse>, ApiError> {
    for token in github_app_installation_tokens(client, provider, credentials).await? {
        let response = github_request(
            client.get(format!(
                "https://api.github.com/repos/{}/{}/branches?per_page=100",
                url_segment(owner),
                url_segment(name)
            )),
            &token,
        )
        .send()
        .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            continue;
        }
        let values = response.error_for_status()?.json::<Vec<Value>>().await?;
        return Ok(branch_responses(values));
    }
    Err(ApiError::BadRequest(
        "repository is not available to the GitHub App",
    ))
}

async fn github_app_installation_tokens(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<Vec<String>, ApiError> {
    let (app_jwt, installation_ids) =
        github_app_installation_context(client, provider, credentials).await?;
    let mut tokens = Vec::with_capacity(installation_ids.len());
    for installation_id in installation_ids {
        tokens.push(github_installation_token(client, installation_id, &app_jwt).await?);
    }
    Ok(tokens)
}

async fn github_app_installation_context(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<(String, Vec<u64>), ApiError> {
    let app_id = provider
        .application_id
        .as_deref()
        .ok_or(ApiError::BadRequest("GitHub App ID is missing"))?;
    let private_key = credentials
        .private_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::BadRequest("GitHub App private key is missing"))?;
    let app_jwt = github_app_jwt(app_id, private_key)?;
    let installation_ids = if let Some(id) = provider.installation_id.as_deref() {
        vec![
            id.parse::<u64>()
                .map_err(|_| ApiError::BadRequest("GitHub installation ID is invalid"))?,
        ]
    } else {
        github_request(
            client.get("https://api.github.com/app/installations?per_page=100"),
            &app_jwt,
        )
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<GithubInstallation>>()
        .await?
        .into_iter()
        .map(|installation| installation.id)
        .collect()
    };
    if installation_ids.is_empty() {
        return Err(ApiError::BadRequest("GitHub App is not installed"));
    }
    Ok((app_jwt, installation_ids))
}

pub(crate) async fn test(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Json<ProviderConnectionResponse>, ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let provider = state
        .database
        .providers()
        .get(&provider_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let credentials = decrypt_credentials(&state, &provider)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(ApiError::ProviderRemote)?;
    let repository_count = test_provider(&client, &provider, &credentials).await?;
    let verified = state
        .database
        .providers()
        .mark_verified(&provider_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let checked_at = verified
        .last_verified_at
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let _ = actor;
    Ok(Json(ProviderConnectionResponse {
        repository_count,
        checked_at,
    }))
}

async fn test_provider(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<Option<u64>, ApiError> {
    match (provider.kind, provider.auth_mode) {
        (ProviderKind::Github, ProviderAuthMode::GithubApp) => {
            Ok(Some(test_github_app(client, provider, credentials).await?))
        }
        (ProviderKind::Github, ProviderAuthMode::Token) => {
            Ok(Some(test_github_token(client, credentials).await?))
        }
        (ProviderKind::Github, ProviderAuthMode::OAuth) if credentials.token.is_some() => {
            Ok(Some(test_github_token(client, credentials).await?))
        }
        (ProviderKind::Gitlab, ProviderAuthMode::Token) => {
            Ok(Some(test_gitlab(client, provider, credentials).await?))
        }
        (ProviderKind::Gitlab, ProviderAuthMode::OAuth) if credentials.token.is_some() => {
            Ok(Some(test_gitlab(client, provider, credentials).await?))
        }
        (ProviderKind::Gitea, ProviderAuthMode::Token) => {
            Ok(Some(test_gitea(client, provider, credentials).await?))
        }
        (ProviderKind::Gitea, ProviderAuthMode::OAuth) if credentials.token.is_some() => {
            Ok(Some(test_gitea(client, provider, credentials).await?))
        }
        (ProviderKind::Git, ProviderAuthMode::Token) => {
            test_generic_git(client, provider, credentials).await
        }
        (_, ProviderAuthMode::OAuth) => Err(ApiError::BadRequest(
            "OAuth provider connection is not completed",
        )),
        (_, ProviderAuthMode::GithubApp) => Err(ApiError::BadRequest(
            "GitHub App mode is only available for GitHub",
        )),
    }
}

async fn test_github_token(
    client: &Client,
    credentials: &StoredCredentials,
) -> Result<u64, ApiError> {
    let token = required_token(credentials)?;
    let response = authorized(client.get(
        "https://api.github.com/user/repos?per_page=1&affiliation=owner,collaborator,organization_member",
    ), ProviderKind::Github, token)
    .send()
    .await?
    .error_for_status()?;
    let headers = response.headers().clone();
    let repositories = response.json::<Vec<Value>>().await?;
    Ok(repository_count(&headers, repositories.len() as u64))
}

async fn test_gitlab(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<u64, ApiError> {
    let token = required_token(credentials)?;
    let response = authorized(
        client.get(provider_endpoint(
            &provider.base_url,
            "/api/v4/projects?membership=true&per_page=1",
        )?),
        ProviderKind::Gitlab,
        token,
    )
    .send()
    .await?
    .error_for_status()?;
    let headers = response.headers().clone();
    let repositories = response.json::<Vec<Value>>().await?;
    Ok(repository_count(&headers, repositories.len() as u64))
}

async fn test_gitea(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<u64, ApiError> {
    let token = required_token(credentials)?;
    let response = authorized(
        client.get(provider_endpoint(
            &provider.base_url,
            "/api/v1/user/repos?limit=1",
        )?),
        ProviderKind::Gitea,
        token,
    )
    .send()
    .await?
    .error_for_status()?;
    let headers = response.headers().clone();
    let repositories = response.json::<Vec<Value>>().await?;
    Ok(repository_count(&headers, repositories.len() as u64))
}

async fn test_generic_git(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<Option<u64>, ApiError> {
    let token = required_token(credentials)?;
    authorized(client.head(&provider.base_url), ProviderKind::Git, token)
        .send()
        .await?
        .error_for_status()?;
    Ok(None)
}

async fn test_github_app(
    client: &Client,
    provider: &ProviderRecord,
    credentials: &StoredCredentials,
) -> Result<u64, ApiError> {
    let mut total = 0;
    for access_token in github_app_installation_tokens(client, provider, credentials).await? {
        let response = github_request(
            client.get("https://api.github.com/installation/repositories?per_page=1"),
            &access_token,
        )
        .send()
        .await?
        .error_for_status()?;
        let headers = response.headers().clone();
        let repositories = response.json::<GithubInstallationRepositories>().await?;
        let body_count = repositories
            .total_count
            .or_else(|| repositories.repositories.map(|items| items.len() as u64))
            .unwrap_or_default();
        total += repository_count(&headers, body_count);
    }
    Ok(total)
}

async fn github_installation_token(
    client: &Client,
    installation_id: u64,
    app_jwt: &str,
) -> Result<String, ApiError> {
    Ok(github_request(
        client.post(format!(
            "https://api.github.com/app/installations/{installation_id}/access_tokens"
        )),
        app_jwt,
    )
    .send()
    .await?
    .error_for_status()?
    .json::<GithubAccessToken>()
    .await?
    .token)
}

fn github_app_jwt(app_id: &str, private_key: &str) -> Result<String, ApiError> {
    let now = Utc::now().timestamp();
    let claims = GithubAppClaims {
        iat: now - 60,
        exp: now + 9 * 60,
        iss: app_id.to_owned(),
    };
    let key = EncodingKey::from_rsa_pem(private_key.as_bytes())
        .map_err(|_| ApiError::BadRequest("GitHub App private key is invalid"))?;
    jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|_| ApiError::BadRequest("GitHub App private key is invalid"))
}

fn decrypt_credentials(
    state: &AppState,
    provider: &ProviderRecord,
) -> Result<StoredCredentials, ApiError> {
    let cipher = state
        .provider_cipher
        .as_ref()
        .ok_or(ApiError::ProviderCapabilityUnavailable)?;
    let plaintext = cipher
        .decrypt(&provider.credentials_ciphertext)
        .map_err(|_| ApiError::ProviderCapabilityUnavailable)?;
    match serde_json::from_slice(plaintext.as_slice()) {
        Ok(credentials) => Ok(credentials),
        Err(_) if provider.auth_mode == ProviderAuthMode::Token => {
            let token = String::from_utf8(plaintext.to_vec())
                .map_err(|_| ApiError::ProviderCapabilityUnavailable)?;
            Ok(StoredCredentials {
                token: Some(token),
                private_key: None,
            })
        }
        Err(_) => Err(ApiError::ProviderCapabilityUnavailable),
    }
}

fn required_token(credentials: &StoredCredentials) -> Result<&str, ApiError> {
    credentials
        .token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
        .ok_or(ApiError::BadRequest("provider access token is missing"))
}

fn authorized(request: RequestBuilder, kind: ProviderKind, token: &str) -> RequestBuilder {
    let request = match kind {
        ProviderKind::Gitea => request.header(header::AUTHORIZATION, format!("token {token}")),
        _ => request.bearer_auth(token),
    };
    request
        .header(header::USER_AGENT, "Ignitify")
        .header(header::ACCEPT, "application/json")
}

fn github_request(request: RequestBuilder, token: &str) -> RequestBuilder {
    request
        .bearer_auth(token)
        .header(header::USER_AGENT, "Ignitify")
        .header(header::ACCEPT, "application/vnd.github+json")
}

fn provider_endpoint(base_url: &str, suffix: &str) -> Result<String, ApiError> {
    let url = Url::parse(base_url).map_err(|_| ApiError::BadRequest("provider URL is invalid"))?;
    let base = base_url.trim_end_matches('/');
    if url.host_str().is_none() {
        return Err(ApiError::BadRequest("provider URL is invalid"));
    }
    Ok(format!("{base}{suffix}"))
}

fn repository_count(headers: &ResponseHeaders, body_count: u64) -> u64 {
    for name in ["x-total-count", "x-total"] {
        if let Some(value) = headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            return value;
        }
    }
    if let Some(link) = headers
        .get(header::LINK)
        .and_then(|value| value.to_str().ok())
    {
        for part in link.split(',') {
            if !part.contains("rel=\"last\"") && !part.contains("rel=last") {
                continue;
            }
            let Some(start) = part.find('<') else {
                continue;
            };
            let Some(end) = part[start + 1..].find('>') else {
                continue;
            };
            let Ok(url) = Url::parse(&part[start + 1..start + 1 + end]) else {
                continue;
            };
            if let Some(page) = url
                .query_pairs()
                .find_map(|(key, value)| (key == "page").then(|| value.parse::<u64>().ok()))
                .flatten()
            {
                return page;
            }
        }
    }
    body_count
}

async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedUser, ApiError> {
    let actor = require_actor(state, headers).await?;
    if actor.has_admin_access() {
        Ok(actor)
    } else {
        Err(ApiError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderMap;

    use super::repository_count;

    #[test]
    fn repository_count_reads_last_page_from_link_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "link",
            "<https://example.test/repos?page=42&per_page=1>; rel=\"last\""
                .parse()
                .unwrap(),
        );
        assert_eq!(repository_count(&headers, 1), 42);
    }
}
