use std::path::Path;

use ignitify_db::{ProviderAuthMode, ProviderKind, ProviderRecord};
use ignitify_domain::ServiceSourceConfig;
use tokio::fs;

use crate::{
    Checkout, GitCredentials, GitSourceBuild, StoredCredentials,
    build_support::BuildError,
    github_app,
    source_spec::{is_git_revision, repository_url},
};

impl GitSourceBuild {
    pub(crate) async fn checkout_source(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
    ) -> Result<Checkout, BuildError> {
        let provider_id = source
            .provider_id
            .as_deref()
            .ok_or(BuildError::InvalidSource)?;
        let repository = source
            .repository
            .as_deref()
            .ok_or(BuildError::InvalidSource)?;
        let branch = source.branch.as_deref().ok_or(BuildError::InvalidSource)?;
        let provider = self
            .database
            .providers()
            .get(provider_id)
            .await?
            .ok_or(BuildError::ProviderMissing)?;
        let credentials = self.credentials(&provider, repository).await?;
        self.checkout(
            deployment.id.as_str(),
            &provider,
            &credentials,
            repository,
            branch,
            deployment.source_revision.as_deref(),
        )
        .await
    }

    async fn credentials(
        &self,
        provider: &ProviderRecord,
        repository: &str,
    ) -> Result<GitCredentials, BuildError> {
        if provider.auth_mode == ProviderAuthMode::GithubApp {
            let plaintext = self
                .cipher
                .decrypt(&provider.credentials_ciphertext)
                .map_err(|_| BuildError::CredentialsUnavailable)?;
            let credentials = serde_json::from_slice::<StoredCredentials>(plaintext.as_slice())
                .map_err(|_| BuildError::CredentialsUnavailable)?;
            let private_key = credentials
                .private_key
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or(BuildError::CredentialsUnavailable)?;
            let token =
                github_app::installation_access_token(provider, private_key, repository).await?;
            return Ok(GitCredentials {
                username: "x-access-token".to_owned(),
                token,
            });
        }
        let plaintext = self
            .cipher
            .decrypt(&provider.credentials_ciphertext)
            .map_err(|_| BuildError::CredentialsUnavailable)?;
        let token = match serde_json::from_slice::<StoredCredentials>(plaintext.as_slice()) {
            Ok(credentials) => credentials.token,
            Err(_) => String::from_utf8(plaintext.to_vec()).ok(),
        }
        .filter(|token| !token.trim().is_empty())
        .ok_or(BuildError::CredentialsUnavailable)?;
        let username = provider
            .username
            .clone()
            .unwrap_or_else(|| match provider.kind {
                ProviderKind::Github => "x-access-token".to_owned(),
                ProviderKind::Gitlab => "oauth2".to_owned(),
                ProviderKind::Gitea | ProviderKind::Git => "git".to_owned(),
            });
        Ok(GitCredentials { username, token })
    }

    async fn checkout(
        &self,
        deployment_id: &str,
        provider: &ProviderRecord,
        credentials: &GitCredentials,
        repository: &str,
        branch: &str,
        source_revision: Option<&str>,
    ) -> Result<Checkout, BuildError> {
        fs::create_dir_all(&self.root).await?;
        let root = fs::canonicalize(&self.root).await?;
        let path = root.join(deployment_id);
        let credentials_path = root.join(format!("{deployment_id}.gitconfig"));
        remove_dir_if_exists(&path).await?;
        remove_file_if_exists(&credentials_path).await?;
        let remote = repository_url(provider, repository)?;
        write_credentials_config(&credentials_path, &crate::git_config(credentials)).await?;
        let clone_result = self
            .run(
                self.git_command(&credentials_path)
                    .args([
                        "clone",
                        "--depth",
                        "1",
                        "--no-tags",
                        "--single-branch",
                        "--no-recurse-submodules",
                        "--branch",
                        branch,
                    ])
                    .arg(&remote)
                    .arg(&path),
                "git checkout",
            )
            .await;
        if let Err(error) = clone_result {
            let _ = fs::remove_file(&credentials_path).await;
            return Err(error);
        }
        if let Some(revision) = source_revision {
            if let Err(error) = self
                .run(
                    self.git_command(&credentials_path).args([
                        "-C",
                        path.to_string_lossy().as_ref(),
                        "fetch",
                        "--depth",
                        "1",
                        "origin",
                        revision,
                    ]),
                    "git revision fetch",
                )
                .await
            {
                let _ = fs::remove_file(&credentials_path).await;
                return Err(error);
            }
            if let Err(error) = self
                .run(
                    self.git_command(&credentials_path).args([
                        "-C",
                        path.to_string_lossy().as_ref(),
                        "checkout",
                        "--detach",
                        "FETCH_HEAD",
                    ]),
                    "git revision checkout",
                )
                .await
            {
                let _ = fs::remove_file(&credentials_path).await;
                return Err(error);
            }
        }
        let revision = self
            .output(
                self.git_command(&credentials_path).args([
                    "-C",
                    path.to_string_lossy().as_ref(),
                    "rev-parse",
                    "HEAD",
                ]),
                "git revision",
            )
            .await;
        let _ = fs::remove_file(&credentials_path).await;
        let revision = revision?;
        if !is_git_revision(&revision) {
            return Err(BuildError::InvalidRevision);
        }
        Ok(Checkout { path, revision })
    }
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

async fn remove_file_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

async fn write_credentials_config(path: &Path, contents: &str) -> Result<(), std::io::Error> {
    crate::sensitive_file::write_sensitive_file(path, contents.as_bytes()).await
}
