use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use hmac::{Hmac, KeyInit, Mac};
use ignitify_control_plane::DeploymentSubmission;
use ignitify_db::{DeploymentActor, ProviderKind};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{error::ApiError, state::AppState};

pub(crate) const WEBHOOK_BODY_LIMIT: usize = 1024 * 1024;

type HmacSha256 = Hmac<Sha256>;

pub(crate) async fn receive_push(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let Some(target) = state
        .services()?
        .auto_deploy_webhook_target(&service_id)
        .await?
    else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let Some(provider) = state.database.providers().get(&target.provider_id).await? else {
        return Ok(StatusCode::NO_CONTENT);
    };
    match verify_push_event(
        provider.kind,
        &headers,
        body.as_ref(),
        target.secret.as_bytes(),
    ) {
        WebhookVerification::Ignored => return Ok(StatusCode::NO_CONTENT),
        WebhookVerification::Invalid => return Ok(StatusCode::UNAUTHORIZED),
        WebhookVerification::Verified => {}
    }
    let Ok(payload) = serde_json::from_slice::<PushPayload>(&body) else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let Some(source_revision) = payload.matches(&target.repository, &target.branch) else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let idempotency_key = webhook_idempotency_key(provider.kind, &headers, body.as_ref());
    let actor = DeploymentActor {
        // The project owner is a durable user principal for this system-triggered action.
        id: &target.project_owner_id,
        is_admin: true,
    };
    match state
        .control()?
        .submit_deploy_with_source_revision(
            actor,
            &target.service_id,
            &idempotency_key,
            Some(source_revision),
        )
        .await?
    {
        DeploymentSubmission::Accepted(_)
        | DeploymentSubmission::Existing(_)
        | DeploymentSubmission::ActiveConflict
        | DeploymentSubmission::Missing
        | DeploymentSubmission::Forbidden => Ok(StatusCode::NO_CONTENT),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebhookVerification {
    Ignored,
    Invalid,
    Verified,
}

fn verify_push_event(
    provider: ProviderKind,
    headers: &HeaderMap,
    body: &[u8],
    secret: &[u8],
) -> WebhookVerification {
    match provider {
        ProviderKind::Github => {
            if header(headers, "x-github-event") != Some("push") {
                return WebhookVerification::Ignored;
            }
            let Some(signature) = header(headers, "x-hub-signature-256")
                .and_then(|value| value.strip_prefix("sha256:"))
            else {
                return WebhookVerification::Invalid;
            };
            signature_matches(secret, body, signature)
        }
        ProviderKind::Gitlab => {
            if header(headers, "x-gitlab-event") != Some("Push Hook") {
                return WebhookVerification::Ignored;
            }
            let Some(token) = header(headers, "x-gitlab-token") else {
                return WebhookVerification::Invalid;
            };
            if secret.ct_eq(token.as_bytes()).into() {
                WebhookVerification::Verified
            } else {
                WebhookVerification::Invalid
            }
        }
        ProviderKind::Gitea => {
            if header(headers, "x-gitea-event") != Some("push") {
                return WebhookVerification::Ignored;
            }
            let Some(signature) = header(headers, "x-gitea-signature") else {
                return WebhookVerification::Invalid;
            };
            signature_matches(secret, body, signature)
        }
        ProviderKind::Git => WebhookVerification::Ignored,
    }
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn signature_matches(secret: &[u8], body: &[u8], signature: &str) -> WebhookVerification {
    let Some(provided) = decode_hex(signature) else {
        return WebhookVerification::Invalid;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return WebhookVerification::Invalid;
    };
    mac.update(body);
    let expected = mac.finalize().into_bytes();
    if expected.as_slice().ct_eq(&provided).into() {
        WebhookVerification::Verified
    } else {
        WebhookVerification::Invalid
    }
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() != 64 || !value.is_ascii() {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

fn webhook_idempotency_key(provider: ProviderKind, headers: &HeaderMap, body: &[u8]) -> String {
    let delivery_header = match provider {
        ProviderKind::Github => "x-github-delivery",
        ProviderKind::Gitlab => "x-gitlab-event-uuid",
        ProviderKind::Gitea => "x-gitea-delivery",
        ProviderKind::Git => "",
    };
    if let Some(delivery_id) = header(headers, delivery_header).filter(is_idempotency_fragment) {
        return format!("webhook:{delivery_id}");
    }
    format!(
        "webhook:sha256:{}",
        hex_encode(Sha256::digest(body).as_ref())
    )
}

fn is_idempotency_fragment(value: &&str) -> bool {
    !value.is_empty()
        && value.len() <= 112
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[derive(Debug, Deserialize)]
struct PushPayload {
    #[serde(rename = "ref")]
    reference: Option<String>,
    after: Option<String>,
    repository: Option<WebhookRepository>,
    project: Option<GitlabProject>,
}

impl PushPayload {
    fn matches<'a>(&'a self, repository: &str, branch: &str) -> Option<&'a str> {
        let payload_repository = self
            .repository
            .as_ref()
            .and_then(WebhookRepository::full_name)
            .or_else(|| {
                self.project
                    .as_ref()
                    .and_then(|project| project.path_with_namespace.as_deref())
            })?;
        if payload_repository != repository
            || self.reference.as_deref()? != format!("refs/heads/{branch}")
        {
            return None;
        }
        let revision = self.after.as_deref()?;
        valid_source_revision(revision).then_some(revision)
    }
}

#[derive(Debug, Deserialize)]
struct WebhookRepository {
    full_name: Option<String>,
    path_with_namespace: Option<String>,
}

impl WebhookRepository {
    fn full_name(&self) -> Option<&str> {
        self.full_name
            .as_deref()
            .or(self.path_with_namespace.as_deref())
    }
}

#[derive(Debug, Deserialize)]
struct GitlabProject {
    path_with_namespace: Option<String>,
}

fn valid_source_revision(value: &str) -> bool {
    (40..=64).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && value.bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
mod tests {
    use super::{
        PushPayload, WebhookVerification, hex_encode, signature_matches, valid_source_revision,
        webhook_idempotency_key,
    };
    use axum::http::{HeaderMap, HeaderValue};
    use hmac::{Hmac, KeyInit, Mac};
    use ignitify_db::ProviderKind;
    use sha2::Sha256;

    #[test]
    fn verifies_a_github_style_hmac_signature() {
        let body = br#"{\"ref\":\"refs/heads/main\"}"#;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(body);
        let signature = hex_encode(mac.finalize().into_bytes().as_slice());

        assert_eq!(
            signature_matches(b"secret", body, &signature),
            WebhookVerification::Verified
        );
        assert_eq!(
            signature_matches(b"wrong", body, &signature),
            WebhookVerification::Invalid
        );
    }

    #[test]
    fn accepts_only_the_configured_repository_branch_and_commit() {
        let payload: PushPayload = serde_json::from_str(
            r#"{
                "ref": "refs/heads/main",
                "after": "0123456789abcdef0123456789abcdef01234567",
                "repository": { "full_name": "acme/site" }
            }"#,
        )
        .unwrap();

        assert_eq!(
            payload.matches("acme/site", "main"),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        assert_eq!(payload.matches("acme/site", "preview"), None);
        assert!(!valid_source_revision(&"0".repeat(40)));
    }

    #[test]
    fn derives_a_retry_stable_key_when_no_delivery_id_is_available() {
        let headers = HeaderMap::new();
        let first = webhook_idempotency_key(ProviderKind::Github, &headers, b"event");
        let second = webhook_idempotency_key(ProviderKind::Github, &headers, b"event");
        assert_eq!(first, second);

        let mut headers = HeaderMap::new();
        headers.insert("x-github-delivery", HeaderValue::from_static("delivery-1"));
        assert_eq!(
            webhook_idempotency_key(ProviderKind::Github, &headers, b"event"),
            "webhook:delivery-1"
        );
    }
}
