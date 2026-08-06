use std::{collections::VecDeque, convert::Infallible, time::Duration};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::stream;
use ignitify_control_plane::StreamRecord;
use ignitify_db::{DeploymentActor, DeploymentEventRecord, DeploymentLogRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

use crate::{
    error::ApiError, extract::require_actor, handlers::deployments::DeploymentResponse,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct StreamQuery {
    after: Option<i64>,
}

#[derive(Debug, Serialize)]
struct Snapshot {
    deployment: DeploymentResponse,
    current_sequence: i64,
}

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    Events,
    Logs,
}

struct StreamState {
    control: ignitify_control_plane::ControlHandle,
    receiver: broadcast::Receiver<StreamRecord>,
    actor_id: String,
    actor_is_admin: bool,
    deployment_id: String,
    kind: StreamKind,
    cursor: i64,
    pending: VecDeque<Event>,
    heartbeat: std::pin::Pin<Box<tokio::time::Sleep>>,
}

pub(crate) async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(deployment_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    open_stream(
        state,
        headers,
        deployment_id,
        query.after,
        StreamKind::Events,
    )
    .await
}

pub(crate) async fn logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(deployment_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    open_stream(state, headers, deployment_id, query.after, StreamKind::Logs).await
}

async fn open_stream(
    state: AppState,
    headers: HeaderMap,
    deployment_id: String,
    query_after: Option<i64>,
    kind: StreamKind,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let after = cursor(&headers, query_after)?;
    let actor_id = actor.id.clone();
    let actor_is_admin = actor.has_admin_access();
    let deployment_actor = DeploymentActor {
        id: &actor_id,
        is_admin: actor_is_admin,
    };
    let control = state.control()?.clone();
    let deployment = control
        .get(deployment_actor, &deployment_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let receiver = control.subscribe();
    let through = match kind {
        StreamKind::Events => control
            .event_cursor(deployment_actor, &deployment_id)
            .await?
            .ok_or(ApiError::NotFound)?
            .newest
            .unwrap_or(after),
        StreamKind::Logs => control
            .log_cursor(deployment_actor, &deployment_id)
            .await?
            .ok_or(ApiError::NotFound)?
            .newest
            .unwrap_or(after),
    };
    let mut pending = VecDeque::new();
    let oldest = match kind {
        StreamKind::Events => {
            let cursor = control
                .event_cursor(deployment_actor, &deployment_id)
                .await?
                .ok_or(ApiError::NotFound)?;
            if cursor.oldest.is_none_or(|oldest| after >= oldest - 1) {
                replay_events(
                    &control,
                    deployment_actor,
                    &deployment_id,
                    after,
                    through,
                    &mut pending,
                )
                .await?;
            }
            cursor.oldest
        }
        StreamKind::Logs => {
            let cursor = control
                .log_cursor(deployment_actor, &deployment_id)
                .await?
                .ok_or(ApiError::NotFound)?;
            if cursor.oldest.is_none_or(|oldest| after >= oldest - 1) {
                replay_logs(
                    &control,
                    deployment_actor,
                    &deployment_id,
                    after,
                    through,
                    &mut pending,
                )
                .await?;
            }
            cursor.oldest
        }
    };
    if let Some(oldest) = oldest
        && after < oldest - 1
    {
        pending.push_front(
            Event::default().event("snapshot").data(
                serde_json::to_string(&Snapshot {
                    deployment: deployment.into(),
                    current_sequence: through,
                })
                .map_err(|_| ApiError::BadRequest("could not encode stream snapshot"))?,
            ),
        );
    }

    let stream = stream::unfold(
        StreamState {
            control,
            receiver,
            actor_id,
            actor_is_admin,
            deployment_id,
            kind,
            cursor: through.max(after),
            pending,
            heartbeat: Box::pin(tokio::time::sleep(Duration::from_secs(15))),
        },
        next_stream_item,
    );
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text(""))
        .into_response();
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/event-stream"),
    );
    Ok(response)
}

async fn replay_events(
    control: &ignitify_control_plane::ControlHandle,
    actor: DeploymentActor<'_>,
    deployment_id: &str,
    after: i64,
    through: i64,
    pending: &mut VecDeque<Event>,
) -> Result<(), ApiError> {
    let records = control
        .events(actor, deployment_id, after, through)
        .await?
        .ok_or(ApiError::NotFound)?;
    for record in records {
        pending.push_back(event_record(record)?);
    }
    Ok(())
}

async fn replay_logs(
    control: &ignitify_control_plane::ControlHandle,
    actor: DeploymentActor<'_>,
    deployment_id: &str,
    after: i64,
    through: i64,
    pending: &mut VecDeque<Event>,
) -> Result<(), ApiError> {
    let records = control
        .logs(actor, deployment_id, after, through)
        .await?
        .ok_or(ApiError::NotFound)?;
    for record in records {
        pending.push_back(log_record(record)?);
    }
    Ok(())
}

async fn next_stream_item(
    mut state: StreamState,
) -> Option<(Result<Event, Infallible>, StreamState)> {
    if let Some(event) = state.pending.pop_front() {
        return Some((Ok(event), state));
    }
    tokio::select! {
        _ = &mut state.heartbeat => {
            state.heartbeat = Box::pin(tokio::time::sleep(Duration::from_secs(15)));
            Some((Ok(Event::default().comment("heartbeat")), state))
        }
        received = state.receiver.recv() => {
            match received {
                Ok(record) => {
                    if let Some(event) = stream_record(&mut state, record).await {
                        Some((Ok(event), state))
                    } else {
                        Some((Ok(Event::default().comment("heartbeat")), state))
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let actor = DeploymentActor { id: &state.actor_id, is_admin: state.actor_is_admin };
                    let cursor = match state.kind {
                        StreamKind::Events => state.control.event_cursor(actor, &state.deployment_id).await.ok().flatten(),
                        StreamKind::Logs => state.control.log_cursor(actor, &state.deployment_id).await.ok().flatten(),
                    }?;
                    let through = cursor.newest.unwrap_or(state.cursor);
                    let replay = match state.kind {
                        StreamKind::Events => state.control.events(actor, &state.deployment_id, state.cursor, through).await.ok().flatten().unwrap_or_default().into_iter().filter_map(|record| event_record(record).ok()).collect::<Vec<_>>(),
                        StreamKind::Logs => state.control.logs(actor, &state.deployment_id, state.cursor, through).await.ok().flatten().unwrap_or_default().into_iter().filter_map(|record| log_record(record).ok()).collect::<Vec<_>>(),
                    };
                    queue_catch_up(&mut state.cursor, &mut state.pending, through, replay);
                    Some((Ok(Event::default().comment("catch-up")), state))
                }
                Err(broadcast::error::RecvError::Closed) => None,
            }
        }
    }
}

fn queue_catch_up(
    cursor: &mut i64,
    pending: &mut VecDeque<Event>,
    through: i64,
    replay: impl IntoIterator<Item = Event>,
) {
    *cursor = (*cursor).max(through);
    pending.extend(replay);
}

async fn stream_record(state: &mut StreamState, record: StreamRecord) -> Option<Event> {
    match (state.kind, record) {
        (StreamKind::Events, StreamRecord::Event(record))
            if record.deployment_id.as_str() == state.deployment_id
                && record.sequence > state.cursor =>
        {
            state.cursor = record.sequence;
            event_record(record).ok()
        }
        (StreamKind::Logs, StreamRecord::Log(record))
            if record.deployment_id.as_str() == state.deployment_id
                && record.sequence > state.cursor =>
        {
            state.cursor = record.sequence;
            log_record(record).ok()
        }
        _ => None,
    }
}

fn event_record(record: DeploymentEventRecord) -> Result<Event, ApiError> {
    let data: Value = serde_json::from_str(&record.payload_json)
        .map_err(|_| ApiError::BadRequest("invalid stored event"))?;
    Ok(Event::default()
        .id(record.sequence.to_string())
        .event(record.kind)
        .data(
            serde_json::json!({
                "sequence": record.sequence,
                "deployment_id": record.deployment_id,
                "created_at": record.created_at,
                "payload": data,
            })
            .to_string(),
        ))
}

fn log_record(record: DeploymentLogRecord) -> Result<Event, ApiError> {
    Ok(Event::default()
        .id(record.sequence.to_string())
        .event("log")
        .data(
            serde_json::json!({
                "sequence": record.sequence,
                "deployment_id": record.deployment_id,
                "stream": record.stream,
                "line": record.line,
                "created_at": record.created_at,
            })
            .to_string(),
        ))
}

fn cursor(headers: &HeaderMap, query_after: Option<i64>) -> Result<i64, ApiError> {
    let cursor = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .or(query_after)
        .unwrap_or(0);
    if cursor < 0 {
        return Err(ApiError::BadRequest("cursor must be non-negative"));
    }
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn invalid_last_event_id_uses_after_query() {
        let mut headers = HeaderMap::new();
        headers.insert("last-event-id", HeaderValue::from_static("invalid"));

        assert_eq!(super::cursor(&headers, Some(7)).unwrap(), 7);
    }

    #[test]
    fn negative_cursor_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert("last-event-id", HeaderValue::from_static("-1"));

        assert!(super::cursor(&headers, None).is_err());
    }

    #[test]
    fn durable_lag_replay_advances_cursor_before_queuing_events() {
        let mut cursor = 7;
        let mut pending = std::collections::VecDeque::new();

        super::queue_catch_up(
            &mut cursor,
            &mut pending,
            11,
            [axum::response::sse::Event::default().event("deployment.running")],
        );

        assert_eq!(cursor, 11);
        assert_eq!(pending.len(), 1);
    }
}
