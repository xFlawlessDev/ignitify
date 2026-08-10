use chrono::Utc;
use ignitify_db::ProviderRecord;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::{Client, RequestBuilder, header};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

const PUBLIC_GITHUB_BASE_URL: &str = "https://github.com";
const PUBLIC_GITHUB_API_URL: &str = "https://api.github.com/";
const USER_AGENT: &str = "Ignitify";

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error("GitHub App ID is missing")]
    AppIdMissing,
    #[error("GitHub App installation ID is invalid")]
    InstallationIdInvalid,
    #[error("GitHub App private key is invalid")]
    PrivateKeyInvalid,
    #[error("repository must use owner/name format")]
    RepositoryInvalid,
    #[error("GitHub App is not installed for this repository")]
    InstallationUnavailable,
    #[error("could not contact the GitHub App API")]
    RequestFailed,
    #[error("GitHub did not return an installation access token")]
    TokenUnavailable,
}

#[derive(Debug, Deserialize)]
struct GithubInstallation {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct GithubAccessToken {
    token: String,
}

#[derive(Debug, Serialize)]
struct GithubAppClaims {
    iat: i64,
    exp: i64,
    iss: String,
}

pub(super) async fn installation_access_token(
    provider: &ProviderRecord,
    private_key: &str,
    repository: &str,
) -> Result<String, Error> {
    let app_id = provider
        .application_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(Error::AppIdMissing)?;
    let app_jwt = app_jwt(app_id, private_key)?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| Error::RequestFailed)?;
    let api_base = github_api_base(&provider.base_url)?;
    let installation_id = match provider.installation_id.as_deref() {
        Some(value) if !value.trim().is_empty() => value
            .parse::<u64>()
            .map_err(|_| Error::InstallationIdInvalid)?,
        _ => repository_installation(&client, &api_base, repository, &app_jwt).await?,
    };
    create_installation_token(&client, &api_base, installation_id, &app_jwt).await
}

fn app_jwt(app_id: &str, private_key: &str) -> Result<String, Error> {
    let now = Utc::now().timestamp();
    let claims = GithubAppClaims {
        iat: now - 60,
        exp: now + 9 * 60,
        iss: app_id.to_owned(),
    };
    let key =
        EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|_| Error::PrivateKeyInvalid)?;
    jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|_| Error::PrivateKeyInvalid)
}

async fn repository_installation(
    client: &Client,
    api_base: &Url,
    repository: &str,
    app_jwt: &str,
) -> Result<u64, Error> {
    let (owner, name) = repository_parts(repository)?;
    let endpoint = api_endpoint(api_base, &format!("repos/{owner}/{name}/installation"))?;
    let response = github_request(client.get(endpoint), app_jwt)
        .send()
        .await
        .map_err(|_| Error::RequestFailed)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::InstallationUnavailable);
    }
    response
        .error_for_status()
        .map_err(|_| Error::RequestFailed)?
        .json::<GithubInstallation>()
        .await
        .map_err(|_| Error::RequestFailed)
        .map(|installation| installation.id)
}

async fn create_installation_token(
    client: &Client,
    api_base: &Url,
    installation_id: u64,
    app_jwt: &str,
) -> Result<String, Error> {
    let endpoint = api_endpoint(
        api_base,
        &format!("app/installations/{installation_id}/access_tokens"),
    )?;
    let token = github_request(client.post(endpoint), app_jwt)
        .send()
        .await
        .map_err(|_| Error::RequestFailed)?
        .error_for_status()
        .map_err(|_| Error::TokenUnavailable)?
        .json::<GithubAccessToken>()
        .await
        .map_err(|_| Error::TokenUnavailable)?
        .token;
    if token.trim().is_empty() {
        Err(Error::TokenUnavailable)
    } else {
        Ok(token)
    }
}

fn github_request(request: RequestBuilder, token: &str) -> RequestBuilder {
    request
        .bearer_auth(token)
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::USER_AGENT, USER_AGENT)
}

fn github_api_base(base_url: &str) -> Result<Url, Error> {
    if base_url.trim_end_matches('/') == PUBLIC_GITHUB_BASE_URL {
        return Url::parse(PUBLIC_GITHUB_API_URL).map_err(|_| Error::RequestFailed);
    }
    let mut base = Url::parse(base_url).map_err(|_| Error::RequestFailed)?;
    if base.scheme() != "https" || base.host_str().is_none() {
        return Err(Error::RequestFailed);
    }
    let path = base.path().trim_end_matches('/');
    base.set_path(&format!("{path}/api/v3/"));
    base.set_query(None);
    base.set_fragment(None);
    Ok(base)
}

fn api_endpoint(base: &Url, path: &str) -> Result<Url, Error> {
    base.join(path).map_err(|_| Error::RequestFailed)
}

fn repository_parts(repository: &str) -> Result<(&str, &str), Error> {
    let Some((owner, name)) = repository.split_once('/') else {
        return Err(Error::RepositoryInvalid);
    };
    if owner.is_empty()
        || name.is_empty()
        || name.contains('/')
        || owner.chars().any(char::is_control)
        || name.chars().any(char::is_control)
    {
        return Err(Error::RepositoryInvalid);
    }
    Ok((owner, name))
}

#[cfg(test)]
mod tests {
    use super::{Error, github_api_base, repository_parts};

    #[test]
    fn uses_the_public_github_api_for_github_com() {
        assert_eq!(
            github_api_base("https://github.com").unwrap().as_str(),
            "https://api.github.com/"
        );
    }

    #[test]
    fn uses_the_enterprise_api_path_for_custom_hosts() {
        assert_eq!(
            github_api_base("https://github.example.com")
                .unwrap()
                .as_str(),
            "https://github.example.com/api/v3/"
        );
    }

    #[test]
    fn repository_parts_require_a_single_owner_separator() {
        assert!(matches!(
            repository_parts("owner/repository"),
            Ok(("owner", "repository"))
        ));
        assert!(matches!(
            repository_parts("owner/repository/extra"),
            Err(Error::RepositoryInvalid)
        ));
    }
}
