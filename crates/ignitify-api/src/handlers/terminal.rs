use std::{sync::Arc, time::Duration};

use axum::{
    extract::{
        ConnectInfo, Extension, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use ignitify_auth::AuthService;
use ignitify_db::AuditOutcome;
use ignitify_runtime_docker::{ContainerTerminalEvent, ContainerTerminalSession};
use ignitify_runtime_remote::{RemoteTerminalEvent, RemoteTerminalSession, SshRuntime};
use ignitify_terminal::{TerminalEvent, TerminalSession};
use serde::{Deserialize, Serialize};

use crate::{
    audit,
    error::ApiError,
    extract::{
        require_trusted_websocket_origin, require_websocket_actor, require_websocket_step_up,
    },
    handlers::runtime::RuntimeDestinationQuery,
    state::AppState,
};

const TERMINAL_PROTOCOL: &str = "ignitify-terminal";
const TERMINAL_REVALIDATION_INTERVAL: Duration = Duration::from_secs(15);
const TERMINAL_MAX_DURATION: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalClientMessage {
    Resize { cols: u16, rows: u16 },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalServerMessage {
    Exited,
    Error { message: &'static str },
}

pub(crate) async fn open(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Query(query): Query<RuntimeDestinationQuery>,
    websocket: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    let websocket_actor = require_websocket_actor(&state, &headers).await?;
    if !websocket_actor.actor.has_platform_operator_access() {
        return Err(ApiError::Forbidden);
    }
    require_trusted_websocket_origin(&state, &headers)?;
    if !requests_terminal_protocol(&headers) {
        return Err(ApiError::BadRequest("invalid terminal protocol"));
    }
    if !state.host_terminal_enabled {
        return Err(ApiError::HostTerminalDisabled);
    }
    require_websocket_step_up(&state, &headers, &websocket_actor.actor).await?;
    let permit = state
        .terminal_sessions
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::TooManyRequests)?;

    if let Some(destination) = query.destination.as_deref() {
        let session = remote_runtime(&state)?
            .open_remote_host_terminal(destination)
            .await
            .map_err(|_| ApiError::RemoteRuntimeFailed)?;
        audit::record(
            &state,
            Some(&websocket_actor.actor),
            &headers,
            peer.as_deref(),
            "terminal.remote_host.open",
            Some("remote_server"),
            Some(destination),
            AuditOutcome::Success,
        )
        .await?;
        let auth = state.auth.clone();
        let bearer_token = websocket_actor.bearer_token;
        return Ok(websocket
            .protocols([TERMINAL_PROTOCOL])
            .on_upgrade(move |socket| serve_remote(socket, session, auth, bearer_token, permit)));
    }

    let session = state.terminal.open()?;
    audit::record(
        &state,
        Some(&websocket_actor.actor),
        &headers,
        peer.as_deref(),
        "terminal.host.open",
        Some("host"),
        None,
        AuditOutcome::Success,
    )
    .await?;
    let auth = state.auth.clone();
    let bearer_token = websocket_actor.bearer_token;
    Ok(websocket
        .protocols([TERMINAL_PROTOCOL])
        .on_upgrade(move |socket| serve(socket, session, auth, bearer_token, permit)))
}

pub(crate) async fn container(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Path(container_id): Path<String>,
    Query(query): Query<RuntimeDestinationQuery>,
    websocket: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    let websocket_actor = require_websocket_actor(&state, &headers).await?;
    if !websocket_actor.actor.has_platform_operator_access() {
        return Err(ApiError::Forbidden);
    }
    require_trusted_websocket_origin(&state, &headers)?;
    if !requests_terminal_protocol(&headers) {
        return Err(ApiError::BadRequest("invalid terminal protocol"));
    }
    let permit = state
        .terminal_sessions
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::TooManyRequests)?;

    if let Some(destination) = query.destination.as_deref() {
        let session = remote_runtime(&state)?
            .open_remote_container_terminal(destination, &container_id)
            .await
            .map_err(|_| ApiError::RemoteRuntimeFailed)?;
        audit::record(
            &state,
            Some(&websocket_actor.actor),
            &headers,
            peer.as_deref(),
            "terminal.remote_container.open",
            Some("container"),
            Some(&container_id),
            AuditOutcome::Success,
        )
        .await?;
        let auth = state.auth.clone();
        let bearer_token = websocket_actor.bearer_token;
        return Ok(websocket
            .protocols([TERMINAL_PROTOCOL])
            .on_upgrade(move |socket| serve_remote(socket, session, auth, bearer_token, permit)));
    }

    let session = state.docker_runtime()?.open_terminal(&container_id).await?;
    audit::record(
        &state,
        Some(&websocket_actor.actor),
        &headers,
        peer.as_deref(),
        "terminal.container.open",
        Some("container"),
        Some(&container_id),
        AuditOutcome::Success,
    )
    .await?;
    let auth = state.auth.clone();
    let bearer_token = websocket_actor.bearer_token;
    Ok(websocket
        .protocols([TERMINAL_PROTOCOL])
        .on_upgrade(move |socket| serve_container(socket, session, auth, bearer_token, permit)))
}

async fn serve(
    socket: WebSocket,
    mut terminal: TerminalSession,
    auth: Arc<AuthService>,
    bearer_token: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut revalidation = tokio::time::interval_at(
        tokio::time::Instant::now() + TERMINAL_REVALIDATION_INTERVAL,
        TERMINAL_REVALIDATION_INTERVAL,
    );
    let timeout = tokio::time::sleep(TERMINAL_MAX_DURATION);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = revalidation.tick() => {
                if !terminal_access_is_valid(&auth, &bearer_token).await {
                    send_server_message(
                        &mut sender,
                        TerminalServerMessage::Error {
                            message: "Terminal session has expired.",
                        },
                    )
                    .await;
                    break;
                }
            }
            _ = &mut timeout => {
                send_server_message(
                    &mut sender,
                    TerminalServerMessage::Error {
                        message: "Terminal session reached its time limit.",
                    },
                )
                .await;
                break;
            }
            event = terminal.next_event() => {
                match event {
                    Some(TerminalEvent::Output(output)) => {
                        if sender.send(Message::Binary(output.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(TerminalEvent::Exited) | None => {
                        send_server_message(&mut sender, TerminalServerMessage::Exited).await;
                        break;
                    }
                    Some(TerminalEvent::Unavailable) => {
                        send_server_message(
                            &mut sender,
                            TerminalServerMessage::Error {
                                message: "Host terminal is unavailable.",
                            },
                        )
                        .await;
                        break;
                    }
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Binary(input))) => {
                        if terminal.input(input.to_vec()).is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(message))) => {
                        if let Ok(TerminalClientMessage::Resize { cols, rows }) = serde_json::from_str(&message)
                            && terminal.resize(cols, rows).is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                }
            }
        }
    }

    terminal.close();
    let _ = sender.send(Message::Close(None)).await;
}

async fn serve_container(
    socket: WebSocket,
    mut terminal: ContainerTerminalSession,
    auth: Arc<AuthService>,
    bearer_token: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut revalidation = tokio::time::interval_at(
        tokio::time::Instant::now() + TERMINAL_REVALIDATION_INTERVAL,
        TERMINAL_REVALIDATION_INTERVAL,
    );
    let timeout = tokio::time::sleep(TERMINAL_MAX_DURATION);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = revalidation.tick() => {
                if !terminal_access_is_valid(&auth, &bearer_token).await {
                    send_server_message(
                        &mut sender,
                        TerminalServerMessage::Error {
                            message: "Terminal session has expired.",
                        },
                    )
                    .await;
                    break;
                }
            }
            _ = &mut timeout => {
                send_server_message(
                    &mut sender,
                    TerminalServerMessage::Error {
                        message: "Terminal session reached its time limit.",
                    },
                )
                .await;
                break;
            }
            event = terminal.next_event() => {
                match event {
                    Ok(Some(ContainerTerminalEvent::Output(output))) => {
                        if sender.send(Message::Binary(output.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(ContainerTerminalEvent::Exited)) | Ok(None) => {
                        send_server_message(&mut sender, TerminalServerMessage::Exited).await;
                        break;
                    }
                    Err(_) => {
                        send_server_message(
                            &mut sender,
                            TerminalServerMessage::Error {
                                message: "Container terminal is unavailable.",
                            },
                        )
                        .await;
                        break;
                    }
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Binary(input))) => {
                        if terminal.input(input.as_ref()).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(message))) => {
                        if let Ok(TerminalClientMessage::Resize { cols, rows }) = serde_json::from_str(&message)
                            && terminal.resize(cols, rows).await.is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                }
            }
        }
    }

    let _ = sender.send(Message::Close(None)).await;
}

async fn serve_remote(
    socket: WebSocket,
    mut terminal: RemoteTerminalSession,
    auth: Arc<AuthService>,
    bearer_token: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut revalidation = tokio::time::interval_at(
        tokio::time::Instant::now() + TERMINAL_REVALIDATION_INTERVAL,
        TERMINAL_REVALIDATION_INTERVAL,
    );
    let timeout = tokio::time::sleep(TERMINAL_MAX_DURATION);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = revalidation.tick() => {
                if !terminal_access_is_valid(&auth, &bearer_token).await {
                    send_server_message(
                        &mut sender,
                        TerminalServerMessage::Error {
                            message: "Terminal session has expired.",
                        },
                    )
                    .await;
                    break;
                }
            }
            _ = &mut timeout => {
                send_server_message(
                    &mut sender,
                    TerminalServerMessage::Error {
                        message: "Terminal session reached its time limit.",
                    },
                )
                .await;
                break;
            }
            event = terminal.next_event() => {
                match event {
                    Some(RemoteTerminalEvent::Output(output)) => {
                        if sender.send(Message::Binary(output.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(RemoteTerminalEvent::Exited) | None => {
                        send_server_message(&mut sender, TerminalServerMessage::Exited).await;
                        break;
                    }
                    Some(RemoteTerminalEvent::Unavailable) => {
                        send_server_message(
                            &mut sender,
                            TerminalServerMessage::Error {
                                message: "Remote terminal is unavailable.",
                            },
                        )
                        .await;
                        break;
                    }
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Binary(input))) => {
                        if terminal.input(input.to_vec()).is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(message))) => {
                        if let Ok(TerminalClientMessage::Resize { cols, rows }) = serde_json::from_str(&message)
                            && terminal.resize(cols, rows).is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                }
            }
        }
    }

    terminal.close();
    let _ = sender.send(Message::Close(None)).await;
}

fn remote_runtime(state: &AppState) -> Result<SshRuntime, ApiError> {
    let cipher = state
        .provider_cipher
        .as_ref()
        .ok_or(ApiError::ProviderCapabilityUnavailable)?;
    Ok(SshRuntime::new(
        state.database.remote_servers(),
        Arc::clone(cipher),
    ))
}

async fn terminal_access_is_valid(auth: &AuthService, bearer_token: &str) -> bool {
    auth.authenticate_bearer(bearer_token)
        .await
        .is_ok_and(|actor| actor.has_platform_operator_access())
}

async fn send_server_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: TerminalServerMessage,
) {
    if let Ok(message) = serde_json::to_string(&message) {
        let _ = sender.send(Message::Text(message.into())).await;
    }
}

fn requests_terminal_protocol(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|protocols| {
            protocols
                .split(',')
                .map(str::trim)
                .any(|protocol| protocol == TERMINAL_PROTOCOL)
        })
}

#[cfg(test)]
mod tests {
    use axum::{http::HeaderValue, http::header::SEC_WEBSOCKET_PROTOCOL};

    #[test]
    fn terminal_protocol_must_be_requested_by_the_client() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("bearer.token, ignitify-terminal"),
        );

        assert!(super::requests_terminal_protocol(&headers));

        headers.insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("bearer.token"),
        );
        assert!(!super::requests_terminal_protocol(&headers));
    }
}
