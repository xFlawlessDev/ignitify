use std::{net::IpAddr, sync::Arc};

use axum::{
    Json,
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::AgeCipher;
use ignitify_db::{
    AuditOutcome, NewNotificationChannel, NotificationChannelRecord, NotificationDeliveryRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::{Host, Url};
use utoipa::ToSchema;

use crate::{
    audit,
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_CHANNEL_NAME_LENGTH: usize = 100;
const MAX_SECRET_LENGTH: usize = 4 * 1024;
const NOTIFICATION_EVENTS: &[&str] = &[
    "deployment.queued",
    "deployment.preparing",
    "deployment.running",
    "deployment.healthy",
    "deployment.failed",
    "deployment.stopping",
    "deployment.stopped",
    "deployment.superseded",
    "backup.succeeded",
    "backup.failed",
    "remote_agent.offline",
    "remote_server.authentication_failed",
    "operations.alert",
];

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NotificationChannelRequest {
    name: String,
    kind: String,
    enabled: bool,
    event_types: Vec<String>,
    #[schema(value_type = Object)]
    configuration: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct NotificationChannelResponse {
    id: String,
    name: String,
    kind: String,
    enabled: bool,
    event_types: Vec<String>,
    #[schema(value_type = Object)]
    configuration_summary: Value,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct NotificationDeliveryQuery {
    limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct NotificationDeliveryResponse {
    id: String,
    channel_id: String,
    channel_name: String,
    channel_kind: String,
    source_kind: String,
    source_id: String,
    event_kind: String,
    status: String,
    attempt_count: i64,
    created_at: String,
    completed_at: Option<String>,
    message: Option<String>,
}

impl From<NotificationDeliveryRecord> for NotificationDeliveryResponse {
    fn from(value: NotificationDeliveryRecord) -> Self {
        Self {
            id: value.id,
            channel_id: value.channel_id,
            channel_name: value.channel_name,
            channel_kind: value.channel_kind,
            source_kind: value.source_kind,
            source_id: value.source_id,
            event_kind: value.event_kind,
            status: value.status,
            attempt_count: value.attempt_count,
            created_at: value.created_at,
            completed_at: value.completed_at,
            message: value.message,
        }
    }
}

impl From<NotificationChannelRecord> for NotificationChannelResponse {
    fn from(value: NotificationChannelRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            enabled: value.enabled,
            event_types: value.event_types,
            configuration_summary: value.configuration_summary,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TelegramConfiguration {
    bot_token: String,
    chat_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscordConfiguration {
    webhook_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmtpConfiguration {
    host: String,
    port: u16,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    from: String,
    to: String,
    #[serde(default = "default_starttls")]
    use_starttls: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResendConfiguration {
    api_key: String,
    from: String,
    to: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WebhookConfiguration {
    url: String,
    #[serde(default)]
    authorization: Option<String>,
}

fn default_starttls() -> bool {
    true
}

#[utoipa::path(
    get,
    path = "/api/v1/notifications",
    tag = "Notifications",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Configured notification channels", body = [NotificationChannelResponse]),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "Platform operator access is required")
    )
)]
pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<NotificationChannelResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    let channels = state
        .database
        .notification_channels()
        .list()
        .await?
        .into_iter()
        .map(NotificationChannelResponse::from)
        .collect();
    Ok(Json(channels))
}

#[utoipa::path(
    get,
    path = "/api/v1/notifications/deliveries",
    tag = "Notifications",
    security(("bearerAuth" = [])),
    params(("limit" = Option<i64>, Query, description = "Maximum records to return (1-100)")),
    responses(
        (status = 200, description = "Recent notification delivery history", body = [NotificationDeliveryResponse]),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "Platform operator access is required")
    )
)]
pub(crate) async fn list_deliveries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<NotificationDeliveryQuery>,
) -> Result<Json<Vec<NotificationDeliveryResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    let deliveries = state
        .database
        .notification_channels()
        .list_deliveries(query.limit.unwrap_or(50))
        .await?
        .into_iter()
        .map(NotificationDeliveryResponse::from)
        .collect();
    Ok(Json(deliveries))
}

#[utoipa::path(
    post,
    path = "/api/v1/notifications",
    tag = "Notifications",
    security(("bearerAuth" = [])),
    params(("X-Ignitify-Request" = String, Header, description = "Required same-origin request marker; use `1`")),
    request_body = NotificationChannelRequest,
    responses(
        (status = 201, description = "Notification channel created", body = NotificationChannelResponse),
        (status = 400, description = "Invalid notification configuration"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "Platform operator access or trusted origin is required"),
        (status = 409, description = "A channel with the same name exists")
    )
)]
pub(crate) async fn create(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<NotificationChannelRequest>,
) -> Result<(StatusCode, Json<NotificationChannelResponse>), ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .notification_channels()
        .create(encrypt_request(&state, request)?)
        .await?;
    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "notification_channel.create",
        Some("notification_channel"),
        Some(&record.id),
        AuditOutcome::Success,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(record.into())))
}

#[utoipa::path(
    put,
    path = "/api/v1/notifications/{notification_id}",
    tag = "Notifications",
    security(("bearerAuth" = [])),
    params(
        ("notification_id" = String, Path, description = "Notification channel identifier"),
        ("X-Ignitify-Request" = String, Header, description = "Required same-origin request marker; use `1`")
    ),
    request_body = NotificationChannelRequest,
    responses(
        (status = 200, description = "Notification channel updated", body = NotificationChannelResponse),
        (status = 400, description = "Invalid notification configuration"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "Platform operator access or trusted origin is required"),
        (status = 404, description = "Notification channel not found"),
        (status = 409, description = "A channel with the same name exists")
    )
)]
pub(crate) async fn update(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
    Json(request): Json<NotificationChannelRequest>,
) -> Result<Json<NotificationChannelResponse>, ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .notification_channels()
        .update(&notification_id, encrypt_request(&state, request)?)
        .await?
        .ok_or(ApiError::NotFound)?;
    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "notification_channel.update",
        Some("notification_channel"),
        Some(&notification_id),
        AuditOutcome::Success,
    )
    .await?;
    Ok(Json(record.into()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/notifications/{notification_id}",
    tag = "Notifications",
    security(("bearerAuth" = [])),
    params(
        ("notification_id" = String, Path, description = "Notification channel identifier"),
        ("X-Ignitify-Request" = String, Header, description = "Required same-origin request marker; use `1`")
    ),
    responses(
        (status = 204, description = "Notification channel removed"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "Platform operator access or trusted origin is required"),
        (status = 404, description = "Notification channel not found")
    )
)]
pub(crate) async fn remove(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if !state
        .database
        .notification_channels()
        .delete(&notification_id)
        .await?
    {
        return Err(ApiError::NotFound);
    }
    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "notification_channel.delete",
        Some("notification_channel"),
        Some(&notification_id),
        AuditOutcome::Success,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn encrypt_request(
    state: &AppState,
    request: NotificationChannelRequest,
) -> Result<NewNotificationChannel, ApiError> {
    let name = normalized_name(request.name)?;
    let kind = normalized_kind(request.kind)?;
    let event_types = normalized_events(request.event_types)?;
    let (configuration, configuration_summary) =
        normalize_configuration(&kind, request.configuration)?;
    let configuration_json = serde_json::to_string(&configuration)
        .map_err(|_| ApiError::BadRequest("notification configuration is invalid"))?;
    let configuration_ciphertext =
        notification_cipher(state)?.encrypt(configuration_json.as_bytes())?;
    Ok(NewNotificationChannel {
        name,
        kind,
        enabled: request.enabled,
        event_types,
        configuration_summary,
        configuration_ciphertext,
    })
}

fn normalized_name(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.chars().count() > MAX_CHANNEL_NAME_LENGTH
        || value.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest(
            "notification channel name must be 1-100 characters",
        ));
    }
    Ok(value)
}

fn normalized_kind(value: String) -> Result<String, ApiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "telegram" | "discord" | "smtp" | "resend" | "webhook" => {
            Ok(value.trim().to_ascii_lowercase())
        }
        _ => Err(ApiError::BadRequest("notification channel kind is invalid")),
    }
}

fn normalized_events(values: Vec<String>) -> Result<Vec<String>, ApiError> {
    if values.is_empty() || values.len() > NOTIFICATION_EVENTS.len() {
        return Err(ApiError::BadRequest(
            "select at least one notification event",
        ));
    }
    let mut events = values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if events
        .iter()
        .any(|event| !NOTIFICATION_EVENTS.contains(&event.as_str()))
    {
        return Err(ApiError::BadRequest("notification event is invalid"));
    }
    events.sort();
    events.dedup();
    if events.is_empty() || events.len() > NOTIFICATION_EVENTS.len() {
        return Err(ApiError::BadRequest(
            "select at least one notification event",
        ));
    }
    Ok(events)
}

fn normalize_configuration(kind: &str, value: Value) -> Result<(Value, Value), ApiError> {
    match kind {
        "telegram" => {
            let configuration: TelegramConfiguration = decode_configuration(value)?;
            let bot_token = token(configuration.bot_token, "Telegram bot token")?;
            let chat_id =
                configuration.chat_id.trim().parse::<i64>().map_err(|_| {
                    ApiError::BadRequest("Telegram chat ID must be a signed integer")
                })?;
            Ok((
                json!({ "bot_token": bot_token, "chat_id": chat_id }),
                json!({ "chat_id": chat_id.to_string() }),
            ))
        }
        "discord" => {
            let configuration: DiscordConfiguration = decode_configuration(value)?;
            let webhook_url = discord_webhook_url(configuration.webhook_url)?;
            Ok((
                json!({ "webhook_url": webhook_url }),
                json!({ "target": "Discord" }),
            ))
        }
        "smtp" => {
            let configuration: SmtpConfiguration = decode_configuration(value)?;
            let host = smtp_host(configuration.host)?;
            if configuration.port == 0 {
                return Err(ApiError::BadRequest("SMTP port is invalid"));
            }
            if !configuration.use_starttls {
                return Err(ApiError::BadRequest("SMTP requires STARTTLS"));
            }
            let username = optional_field(configuration.username, "SMTP username", 320)?;
            let password = optional_secret(configuration.password, "SMTP password")?;
            if username.is_some() != password.is_some() {
                return Err(ApiError::BadRequest(
                    "SMTP username and password must be provided together",
                ));
            }
            let from = email(configuration.from, "SMTP sender")?;
            let to = email(configuration.to, "SMTP recipient")?;
            Ok((
                json!({
                    "host": host,
                    "port": configuration.port,
                    "username": username,
                    "password": password,
                    "from": from,
                    "to": to,
                    "use_starttls": configuration.use_starttls,
                }),
                json!({
                    "host": host,
                    "port": configuration.port,
                    "from": from,
                    "to": to,
                    "authentication_configured": username.is_some(),
                }),
            ))
        }
        "resend" => {
            let configuration: ResendConfiguration = decode_configuration(value)?;
            let api_key = token(configuration.api_key, "Resend API key")?;
            let from = email(configuration.from, "Resend sender")?;
            let to = email(configuration.to, "Resend recipient")?;
            Ok((
                json!({ "api_key": api_key, "from": from, "to": to }),
                json!({ "from": from, "to": to }),
            ))
        }
        "webhook" => {
            let configuration: WebhookConfiguration = decode_configuration(value)?;
            let url = custom_webhook_url(configuration.url)?;
            let authorization =
                optional_secret(configuration.authorization, "webhook authorization")?;
            let host = Url::parse(&url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .ok_or(ApiError::BadRequest("custom webhook URL is invalid"))?;
            Ok((
                json!({ "url": url, "authorization": authorization }),
                json!({ "host": host, "authorization_configured": authorization.is_some() }),
            ))
        }
        _ => Err(ApiError::BadRequest("notification channel kind is invalid")),
    }
}

fn decode_configuration<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, ApiError> {
    serde_json::from_value(value)
        .map_err(|_| ApiError::BadRequest("notification configuration is invalid"))
}

fn discord_webhook_url(value: String) -> Result<String, ApiError> {
    let url = https_url(value, "Discord webhook URL")?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !matches!(host.as_str(), "discord.com" | "discordapp.com")
        || !url.path().starts_with("/api/webhooks/")
    {
        return Err(ApiError::BadRequest("Discord webhook URL is invalid"));
    }
    Ok(url.to_string())
}

fn custom_webhook_url(value: String) -> Result<String, ApiError> {
    let url = https_url(value, "custom webhook URL")?;
    reject_private_host(url.host_str())?;
    Ok(url.to_string())
}

fn https_url(value: String, label: &'static str) -> Result<Url, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > 2_048 {
        return Err(ApiError::BadRequest(match label {
            "Discord webhook URL" => "Discord webhook URL is invalid",
            _ => "custom webhook URL is invalid",
        }));
    }
    let url = Url::parse(&value).map_err(|_| {
        ApiError::BadRequest(match label {
            "Discord webhook URL" => "Discord webhook URL is invalid",
            _ => "custom webhook URL is invalid",
        })
    })?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::BadRequest(match label {
            "Discord webhook URL" => "Discord webhook URL is invalid",
            _ => "custom webhook URL is invalid",
        }));
    }
    Ok(url)
}

fn smtp_host(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || value.len() > 253 || Host::parse(&value).is_err() {
        return Err(ApiError::BadRequest("SMTP host is invalid"));
    }
    Ok(value)
}

fn email(value: String, label: &'static str) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.len() > 320
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
        || !value.contains('@')
    {
        return Err(ApiError::BadRequest(match label {
            "SMTP sender" => "SMTP sender email is invalid",
            "SMTP recipient" => "SMTP recipient email is invalid",
            "Resend sender" => "Resend sender email is invalid",
            _ => "Resend recipient email is invalid",
        }));
    }
    Ok(value)
}

fn optional_field(
    value: Option<String>,
    label: &'static str,
    maximum: usize,
) -> Result<Option<String>, ApiError> {
    value
        .map(|value| {
            let value = value.trim().to_owned();
            if value.is_empty() {
                return Ok(None);
            }
            if value.len() > maximum || value.chars().any(char::is_control) {
                return Err(ApiError::BadRequest(match label {
                    "SMTP username" => "SMTP username is invalid",
                    _ => "notification field is invalid",
                }));
            }
            Ok(Some(value))
        })
        .unwrap_or(Ok(None))
}

fn token(value: String, label: &'static str) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.len() > MAX_SECRET_LENGTH
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
    {
        return Err(ApiError::BadRequest(match label {
            "Telegram bot token" => "Telegram bot token is invalid",
            "Resend API key" => "Resend API key is invalid",
            _ => "notification credential is invalid",
        }));
    }
    Ok(value)
}

fn secret(value: String, label: &'static str) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > MAX_SECRET_LENGTH || value.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(match label {
            "SMTP password" => "SMTP password is invalid",
            _ => "webhook authorization is invalid",
        }));
    }
    Ok(value)
}

fn optional_secret(value: Option<String>, label: &'static str) -> Result<Option<String>, ApiError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| secret(value, label))
        .transpose()
}

fn reject_private_host(host: Option<&str>) -> Result<(), ApiError> {
    let Some(host) = host else {
        return Err(ApiError::BadRequest("custom webhook URL is invalid"));
    };
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return Err(ApiError::BadRequest(
            "private custom webhook hosts are not allowed",
        ));
    }
    if let Ok(address) = host.parse::<IpAddr>()
        && (address.is_loopback()
            || address.is_unspecified()
            || address.is_multicast()
            || is_private_ip(address))
    {
        return Err(ApiError::BadRequest(
            "private custom webhook hosts are not allowed",
        ));
    }
    Ok(())
}

fn is_private_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => value.is_private() || value.is_link_local(),
        IpAddr::V6(value) => value.is_unique_local() || value.is_unicast_link_local(),
    }
}

async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<ignitify_auth::AuthenticatedUser, ApiError> {
    let actor = require_actor(state, headers).await?;
    if actor.has_platform_operator_access() {
        Ok(actor)
    } else {
        Err(ApiError::Forbidden)
    }
}

fn notification_cipher(state: &AppState) -> Result<&Arc<AgeCipher>, ApiError> {
    state
        .provider_cipher
        .as_ref()
        .ok_or(ApiError::ProviderCapabilityUnavailable)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{normalize_configuration, normalized_events};

    #[test]
    fn accepts_selected_known_events_once() {
        let events = normalized_events(vec![
            "backup.failed".to_owned(),
            "remote_agent.offline".to_owned(),
            "deployment.healthy".to_owned(),
            "backup.failed".to_owned(),
        ])
        .unwrap();
        assert_eq!(
            events,
            [
                "backup.failed",
                "deployment.healthy",
                "remote_agent.offline"
            ]
        );
    }

    #[test]
    fn does_not_include_webhook_authorization_in_summary() {
        let (_, summary) = normalize_configuration(
            "webhook",
            json!({
                "url": "https://events.example.com/ignitify",
                "authorization": "Bearer notification-secret"
            }),
        )
        .unwrap();
        assert_eq!(summary["host"], "events.example.com");
        assert!(summary.get("authorization").is_none());
    }

    #[test]
    fn does_not_include_telegram_token_in_summary() {
        let (_, summary) = normalize_configuration(
            "telegram",
            json!({
                "bot_token": "123456:telegram-token",
                "chat_id": "-100123456"
            }),
        )
        .unwrap();
        assert_eq!(summary["chat_id"], "-100123456");
        assert!(summary.get("bot_token").is_none());
    }
}
