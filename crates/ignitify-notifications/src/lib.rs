//! Outbound notification delivery adapters and event dispatching.

mod operational_alerts;

use std::{net::IpAddr, str::FromStr, sync::Arc, time::Duration};

use ignitify_control_plane::{AgeCipher, StreamPublisher, StreamRecord};
use ignitify_db::{Database, NotificationChannelConnection, RemoteNotificationEventRecord};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use reqwest::{Client, header::HeaderValue, redirect::Policy};
use resend_rs::{Config as ResendConfig, Resend, types::CreateEmailBaseOptions};
use serde::Deserialize;
use serde_json::json;
use teloxide::{
    Bot,
    prelude::{Request, Requester},
    types::ChatId,
};
use thiserror::Error;
use tokio::sync::broadcast;
use url::Url;

pub use operational_alerts::spawn_operational_alert_dispatcher;

const DELIVERY_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_DELIVERY_ATTEMPTS: u8 = 3;
const REMOTE_EVENT_POLL_INTERVAL: Duration = Duration::from_secs(15);
const USER_AGENT: &str = "Ignitify notifications";

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Control(#[from] ignitify_control_plane::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy)]
pub enum BackupStatus {
    Succeeded,
    Failed,
}

pub fn spawn_deployment_dispatcher(
    database: Database,
    cipher: Arc<AgeCipher>,
    publisher: StreamPublisher,
) -> tokio::task::JoinHandle<()> {
    let mut receiver = publisher.subscribe();
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(StreamRecord::Event(event)) => {
                    let notification = NotificationEvent {
                        source_kind: "deployment",
                        source_id: &event.event_id,
                        correlation_id: Some(&event.correlation_id),
                        event_kind: &event.kind,
                        occurred_at: Some(&event.created_at),
                        title: deployment_title(&event.kind),
                        body: deployment_body(&event.deployment_id.to_string(), &event.kind),
                    };
                    if let Err(error) = dispatch(&database, cipher.as_ref(), notification).await {
                        tracing::warn!(error = %error, "notification dispatcher could not prepare a deployment event");
                    }
                }
                Ok(StreamRecord::Log(_)) => {}
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        skipped,
                        "notification dispatcher lagged behind deployment events"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    })
}

pub fn spawn_remote_event_dispatcher(
    database: Database,
    cipher: Arc<AgeCipher>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(REMOTE_EVENT_POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match database
                .remote_server_agents()
                .notification_events(100)
                .await
            {
                Ok(events) => {
                    for event in events {
                        match dispatch_remote_event(&database, cipher.as_ref(), &event).await {
                            Ok(()) => {
                                if let Err(error) = database
                                    .remote_server_agents()
                                    .finish_notification_event(&event.id)
                                    .await
                                {
                                    tracing::warn!(error = %error, event_id = %event.id, "notification dispatcher could not finish a remote event");
                                }
                            }
                            Err(error) => {
                                tracing::warn!(error = %error, event_id = %event.id, "notification dispatcher could not prepare a remote event");
                            }
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "notification dispatcher could not load remote events")
                }
            }
        }
    })
}

pub async fn dispatch_backup(
    database: &Database,
    cipher: &AgeCipher,
    run_id: &str,
    trigger: &str,
    status: BackupStatus,
) -> Result<()> {
    let (event_kind, title, body) = match status {
        BackupStatus::Succeeded => (
            "backup.succeeded",
            "Backup completed",
            format!("Ignitify {trigger} backup completed."),
        ),
        BackupStatus::Failed => (
            "backup.failed",
            "Backup failed",
            format!("Ignitify {trigger} backup failed. Review the server logs."),
        ),
    };
    dispatch(
        database,
        cipher,
        NotificationEvent {
            source_kind: "backup",
            source_id: run_id,
            correlation_id: Some(run_id),
            event_kind,
            occurred_at: None,
            title,
            body,
        },
    )
    .await
}

async fn dispatch_remote_event(
    database: &Database,
    cipher: &AgeCipher,
    event: &RemoteNotificationEventRecord,
) -> Result<()> {
    dispatch(
        database,
        cipher,
        NotificationEvent {
            source_kind: "remote",
            source_id: &event.id,
            correlation_id: Some(&event.id),
            event_kind: &event.kind,
            occurred_at: Some(&event.created_at),
            title: remote_event_title(&event.kind),
            body: remote_event_body(&event.server_name, &event.message),
        },
    )
    .await
}

pub(crate) struct NotificationEvent<'a> {
    pub(crate) source_kind: &'a str,
    pub(crate) source_id: &'a str,
    pub(crate) correlation_id: Option<&'a str>,
    pub(crate) event_kind: &'a str,
    pub(crate) occurred_at: Option<&'a str>,
    pub(crate) title: &'a str,
    pub(crate) body: String,
}

pub(crate) async fn dispatch(
    database: &Database,
    cipher: &AgeCipher,
    event: NotificationEvent<'_>,
) -> Result<()> {
    let channels = database
        .notification_channels()
        .enabled_for_event(event.event_kind)
        .await?;
    for channel in channels {
        dispatch_channel(database, cipher, &channel, &event).await?;
    }
    Ok(())
}

async fn dispatch_channel(
    database: &Database,
    cipher: &AgeCipher,
    channel: &NotificationChannelConnection,
    event: &NotificationEvent<'_>,
) -> Result<()> {
    if event
        .occurred_at
        .is_some_and(|occurred_at| occurred_at < channel.channel.created_at.as_str())
    {
        return Ok(());
    }
    if !database
        .notification_channels()
        .claim_delivery_with_correlation(
            &channel.channel.id,
            event.source_kind,
            event.source_id,
            event.event_kind,
            event.correlation_id,
        )
        .await?
    {
        return Ok(());
    }

    let mut succeeded = false;
    for attempt in 0..MAX_DELIVERY_ATTEMPTS {
        database
            .notification_channels()
            .increment_delivery_attempt(
                &channel.channel.id,
                event.source_kind,
                event.source_id,
                event.event_kind,
            )
            .await?;
        if deliver(cipher, channel, event).await.is_ok() {
            succeeded = true;
            break;
        }
        if attempt + 1 < MAX_DELIVERY_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(250 * 2u64.pow(attempt.into()))).await;
        }
    }
    database
        .notification_channels()
        .finish_delivery(
            &channel.channel.id,
            event.source_kind,
            event.source_id,
            event.event_kind,
            succeeded,
        )
        .await?;
    if !succeeded {
        tracing::warn!(
            channel_id = %channel.channel.id,
            kind = %channel.channel.kind,
            event = event.event_kind,
            "notification delivery failed"
        );
    }
    Ok(())
}

async fn deliver(
    cipher: &AgeCipher,
    channel: &NotificationChannelConnection,
    event: &NotificationEvent<'_>,
) -> std::result::Result<(), ()> {
    let plaintext = cipher
        .decrypt(&channel.configuration_ciphertext)
        .map_err(|_| ())?;
    let result = match channel.channel.kind.as_str() {
        "telegram" => telegram(plaintext.as_slice(), &event.body).await,
        "discord" => discord(plaintext.as_slice(), &event.body).await,
        "smtp" => smtp(plaintext.as_slice(), event.title, &event.body).await,
        "resend" => resend(plaintext.as_slice(), event.title, &event.body).await,
        "webhook" => webhook(plaintext.as_slice(), event).await,
        _ => Err(()),
    };
    drop(plaintext);
    result
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TelegramConfiguration {
    bot_token: String,
    chat_id: i64,
}

async fn telegram(configuration: &[u8], body: &str) -> std::result::Result<(), ()> {
    let configuration: TelegramConfiguration =
        serde_json::from_slice(configuration).map_err(|_| ())?;
    let bot = Bot::new(configuration.bot_token);
    tokio::time::timeout(
        DELIVERY_TIMEOUT,
        bot.send_message(ChatId(configuration.chat_id), bounded_message(body))
            .send(),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscordConfiguration {
    webhook_url: String,
}

async fn discord(configuration: &[u8], body: &str) -> std::result::Result<(), ()> {
    let configuration: DiscordConfiguration =
        serde_json::from_slice(configuration).map_err(|_| ())?;
    let client = http_client()?;
    let response = client
        .post(configuration.webhook_url)
        .json(&json!({ "content": bounded_message(body) }))
        .send()
        .await
        .map_err(|_| ())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SmtpConfiguration {
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    from: String,
    to: String,
    use_starttls: bool,
}

async fn smtp(configuration: &[u8], subject: &str, body: &str) -> std::result::Result<(), ()> {
    let configuration: SmtpConfiguration = serde_json::from_slice(configuration).map_err(|_| ())?;
    if !configuration.use_starttls
        || configuration.username.is_some() != configuration.password.is_some()
    {
        return Err(());
    }
    let from = Mailbox::from_str(&configuration.from).map_err(|_| ())?;
    let to = Mailbox::from_str(&configuration.to).map_err(|_| ())?;
    let message = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .body(body.to_owned())
        .map_err(|_| ())?;
    let mut transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&configuration.host)
        .map_err(|_| ())?
        .port(configuration.port);
    if let (Some(username), Some(password)) = (configuration.username, configuration.password) {
        transport = transport.credentials(Credentials::new(username, password));
    }
    tokio::time::timeout(DELIVERY_TIMEOUT, transport.build().send(message))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResendConfiguration {
    api_key: String,
    from: String,
    to: String,
}

async fn resend(configuration: &[u8], subject: &str, body: &str) -> std::result::Result<(), ()> {
    let configuration: ResendConfiguration =
        serde_json::from_slice(configuration).map_err(|_| ())?;
    let resend_base_url = Url::parse("https://api.resend.com").map_err(|_| ())?;
    let resend = Resend::with_config(
        ResendConfig::builder(configuration.api_key)
            .base_url(resend_base_url)
            .build(),
    );
    let email = CreateEmailBaseOptions::new(configuration.from, [configuration.to], subject)
        .with_text(body);
    tokio::time::timeout(DELIVERY_TIMEOUT, resend.emails.send(email))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WebhookConfiguration {
    url: String,
    authorization: Option<String>,
}

async fn webhook(
    configuration: &[u8],
    event: &NotificationEvent<'_>,
) -> std::result::Result<(), ()> {
    let configuration: WebhookConfiguration =
        serde_json::from_slice(configuration).map_err(|_| ())?;
    ensure_public_https_target(&configuration.url).await?;
    let client = http_client()?;
    let mut request = client.post(configuration.url).json(&json!({
        "event": event.event_kind,
        "source": { "type": event.source_kind, "id": event.source_id },
        "correlation_id": event.correlation_id,
        "title": event.title,
        "message": event.body,
    }));
    if let Some(authorization) = configuration.authorization {
        request = request.header(
            "authorization",
            HeaderValue::from_str(&authorization).map_err(|_| ())?,
        );
    }
    let response = request.send().await.map_err(|_| ())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(())
    }
}

fn http_client() -> std::result::Result<Client, ()> {
    Client::builder()
        .timeout(DELIVERY_TIMEOUT)
        .redirect(Policy::none())
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| ())
}

async fn ensure_public_https_target(value: &str) -> std::result::Result<(), ()> {
    let url = Url::parse(value).map_err(|_| ())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(());
    }
    let host = url.host_str().ok_or(())?;
    let port = url.port_or_known_default().ok_or(())?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ())?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| is_private_ip(address.ip())) {
        return Err(());
    }
    Ok(())
}

fn is_private_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => {
            value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_unspecified()
                || value.is_multicast()
        }
        IpAddr::V6(value) => {
            value.is_unique_local()
                || value.is_loopback()
                || value.is_unicast_link_local()
                || value.is_unspecified()
                || value.is_multicast()
        }
    }
}

fn deployment_title(kind: &str) -> &'static str {
    match kind {
        "deployment.rollback_requested" => "Deployment rollback requested",
        "deployment.healthy" => "Deployment healthy",
        "deployment.failed" => "Deployment failed",
        "deployment.stopped" => "Deployment stopped",
        "deployment.superseded" => "Deployment superseded",
        _ => "Deployment event",
    }
}

fn deployment_body(deployment_id: &str, kind: &str) -> String {
    format!("Ignitify deployment {deployment_id} emitted {kind}.")
}

fn remote_event_title(kind: &str) -> &'static str {
    match kind {
        "remote_agent.offline" => "Remote agent offline",
        "remote_server.authentication_failed" => "Remote SSH authentication failing",
        _ => "Remote runtime event",
    }
}

fn remote_event_body(server_name: &str, message: &str) -> String {
    format!("Ignitify remote server {server_name}: {message}.")
}

fn bounded_message(value: &str) -> String {
    value.chars().take(4_000).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_message, deployment_body, deployment_title, remote_event_body, remote_event_title,
    };

    #[test]
    fn deployment_notification_content_is_bounded_and_non_sensitive() {
        assert_eq!(deployment_title("deployment.healthy"), "Deployment healthy");
        assert_eq!(
            deployment_title("deployment.rollback_requested"),
            "Deployment rollback requested"
        );
        assert_eq!(
            deployment_body("deploy-123", "deployment.failed"),
            "Ignitify deployment deploy-123 emitted deployment.failed."
        );
        assert_eq!(bounded_message(&"x".repeat(5_000)).chars().count(), 4_000);
    }

    #[test]
    fn remote_notification_content_is_bounded_and_non_sensitive() {
        assert_eq!(
            remote_event_title("remote_agent.offline"),
            "Remote agent offline"
        );
        assert_eq!(
            remote_event_body("Production VM", "agent heartbeat timed out"),
            "Ignitify remote server Production VM: agent heartbeat timed out."
        );
    }
}
