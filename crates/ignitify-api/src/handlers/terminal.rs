use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use ignitify_terminal::{TerminalEvent, TerminalSession};
use serde::{Deserialize, Serialize};

use crate::{
    error::ApiError,
    extract::{require_trusted_websocket_origin, require_websocket_actor},
    state::AppState,
};

const TERMINAL_PROTOCOL: &str = "ignitify-terminal";

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
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    let actor = require_websocket_actor(&state, &headers).await?;
    if !actor.has_admin_access() {
        return Err(ApiError::Forbidden);
    }
    require_trusted_websocket_origin(&state, &headers)?;
    if !requests_terminal_protocol(&headers) {
        return Err(ApiError::BadRequest("invalid terminal protocol"));
    }

    let session = state.terminal.open()?;
    Ok(websocket
        .protocols([TERMINAL_PROTOCOL])
        .on_upgrade(move |socket| serve(socket, session)))
}

async fn serve(socket: WebSocket, mut terminal: TerminalSession) {
    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
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
