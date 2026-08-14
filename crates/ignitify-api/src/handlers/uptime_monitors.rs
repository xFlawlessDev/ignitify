use std::net::IpAddr;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_db::{
    NewUptimeMonitor, UPTIME_HISTORY_RETENTION_DAYS, UptimeMonitorHistory, UptimeMonitorRecord,
    UptimeMonitorUpdate,
};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_NAME_LENGTH: usize = 120;
const MAX_TARGET_LENGTH: usize = 2_048;
const MIN_INTERVAL_SECONDS: u32 = 30;
const MAX_INTERVAL_SECONDS: u32 = 86_400;
const DEFAULT_HISTORY_WINDOW_HOURS: u32 = 24;
const MAX_HISTORY_RESPONSE_ROWS: u32 = 500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UptimeMonitorRequest {
    name: String,
    target: String,
    kind: String,
    interval_seconds: u32,
    enabled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UptimeMonitorResponse {
    id: String,
    name: String,
    target: String,
    kind: String,
    interval_seconds: i64,
    enabled: bool,
    status: String,
    history: Vec<String>,
    latency_ms: Option<i64>,
    last_checked_at: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UptimeHistoryQuery {
    hours: Option<u32>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UptimeMonitorHistoryResponse {
    monitor_id: String,
    retention_days: i64,
    checks: Vec<UptimeCheckResponse>,
    summary: UptimeAvailabilityResponse,
}

#[derive(Debug, Serialize)]
struct UptimeCheckResponse {
    status: String,
    latency_ms: Option<i64>,
    error: Option<String>,
    checked_at: String,
}

#[derive(Debug, Serialize)]
struct UptimeAvailabilityResponse {
    window_hours: u32,
    total_checks: i64,
    successful_checks: i64,
    failed_checks: i64,
    availability_percentage: Option<f64>,
    error_budget_percentage: Option<f64>,
    budget_consumed_percentage: Option<f64>,
    status: String,
}

impl From<UptimeMonitorRecord> for UptimeMonitorResponse {
    fn from(record: UptimeMonitorRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            target: record.target,
            kind: record.kind,
            interval_seconds: record.interval_seconds,
            enabled: record.enabled,
            status: record.status,
            history: record.history,
            latency_ms: record.latency_ms,
            last_checked_at: record.last_checked_at,
            last_error: record.last_error,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl UptimeMonitorHistoryResponse {
    fn from_history(monitor_id: String, history: UptimeMonitorHistory) -> Self {
        Self {
            monitor_id,
            retention_days: UPTIME_HISTORY_RETENTION_DAYS,
            checks: history
                .checks
                .into_iter()
                .map(|check| UptimeCheckResponse {
                    status: check.status,
                    latency_ms: check.latency_ms,
                    error: check.error,
                    checked_at: check.checked_at,
                })
                .collect(),
            summary: UptimeAvailabilityResponse {
                window_hours: history.summary.window_hours,
                total_checks: history.summary.total_checks,
                successful_checks: history.summary.successful_checks,
                failed_checks: history.summary.failed_checks,
                availability_percentage: history.summary.availability_percentage,
                error_budget_percentage: history.summary.error_budget_percentage,
                budget_consumed_percentage: history.summary.budget_consumed_percentage,
                status: history.summary.status,
            },
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UptimeMonitorResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let records = state
        .database
        .uptime_monitors()
        .list_for_user(&actor.id)
        .await?
        .into_iter()
        .map(UptimeMonitorResponse::from)
        .collect();
    Ok(Json(records))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UptimeMonitorRequest>,
) -> Result<(StatusCode, Json<UptimeMonitorResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = normalize_request(request)?;
    let record = state
        .database
        .uptime_monitors()
        .create(NewUptimeMonitor {
            user_id: actor.id,
            name: input.name,
            target: input.target,
            kind: input.kind,
            interval_seconds: input.interval_seconds,
            enabled: input.enabled,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(record.into())))
}

pub(crate) async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(monitor_id): Path<String>,
    Query(query): Query<UptimeHistoryQuery>,
) -> Result<Json<UptimeMonitorHistoryResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let hours = query.hours.unwrap_or(DEFAULT_HISTORY_WINDOW_HOURS);
    if hours == 0 || hours > (UPTIME_HISTORY_RETENTION_DAYS * 24) as u32 {
        return Err(ApiError::BadRequest("monitor history window is invalid"));
    }
    let limit = query.limit.unwrap_or(100);
    if limit == 0 || limit > MAX_HISTORY_RESPONSE_ROWS {
        return Err(ApiError::BadRequest("monitor history limit is invalid"));
    }
    let history = state
        .database
        .uptime_monitors()
        .history_for_user(&actor.id, &monitor_id, hours, limit)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(UptimeMonitorHistoryResponse::from_history(
        monitor_id, history,
    )))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(monitor_id): Path<String>,
    Json(request): Json<UptimeMonitorRequest>,
) -> Result<Json<UptimeMonitorResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = normalize_request(request)?;
    let record = state
        .database
        .uptime_monitors()
        .update(
            &actor.id,
            &monitor_id,
            UptimeMonitorUpdate {
                name: input.name,
                target: input.target,
                kind: input.kind,
                interval_seconds: input.interval_seconds,
                enabled: input.enabled,
            },
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(record.into()))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(monitor_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if !state
        .database
        .uptime_monitors()
        .delete(&actor.id, &monitor_id)
        .await?
    {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

struct NormalizedRequest {
    name: String,
    target: String,
    kind: String,
    interval_seconds: u32,
    enabled: bool,
}

fn normalize_request(request: UptimeMonitorRequest) -> Result<NormalizedRequest, ApiError> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > MAX_NAME_LENGTH || name.chars().any(char::is_control) {
        return Err(ApiError::BadRequest("monitor name is invalid"));
    }
    if !(MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&request.interval_seconds) {
        return Err(ApiError::BadRequest("monitor interval is invalid"));
    }
    let (kind, target) = match request.kind.as_str() {
        "http" => ("http", normalize_http_target(&request.target)?),
        "tcp" => ("tcp", normalize_tcp_target(&request.target)?),
        _ => return Err(ApiError::BadRequest("monitor type is invalid")),
    };
    Ok(NormalizedRequest {
        name: name.to_owned(),
        target,
        kind: kind.to_owned(),
        interval_seconds: request.interval_seconds,
        enabled: request.enabled,
    })
}

fn normalize_http_target(value: &str) -> Result<String, ApiError> {
    if value.len() > MAX_TARGET_LENGTH {
        return Err(ApiError::BadRequest("monitor target is too long"));
    }
    let candidate = value.trim();
    let candidate = if candidate.contains("://") {
        candidate.to_owned()
    } else {
        format!("https://{candidate}")
    };
    let url = Url::parse(&candidate).map_err(|_| ApiError::BadRequest("monitor URL is invalid"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::BadRequest("monitor URL is invalid"));
    }
    reject_private_host(url.host_str())?;
    Ok(url.to_string())
}

fn normalize_tcp_target(value: &str) -> Result<String, ApiError> {
    if value.len() > MAX_TARGET_LENGTH {
        return Err(ApiError::BadRequest("monitor target is too long"));
    }
    let (host, port) = value
        .trim()
        .rsplit_once(':')
        .ok_or(ApiError::BadRequest("TCP monitor target must be HOST:PORT"))?;
    if host.is_empty() || host.contains(':') {
        return Err(ApiError::BadRequest("TCP monitor hostname is invalid"));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| ApiError::BadRequest("TCP monitor port is invalid"))?;
    if Host::parse(host).is_err() {
        return Err(ApiError::BadRequest("TCP monitor hostname is invalid"));
    }
    reject_private_host(Some(host))?;
    Ok(format!("{}:{port}", host.to_ascii_lowercase()))
}

fn reject_private_host(host: Option<&str>) -> Result<(), ApiError> {
    let Some(host) = host else {
        return Err(ApiError::BadRequest("monitor hostname is invalid"));
    };
    let value = host.to_string().to_ascii_lowercase();
    if value == "localhost" || value.ends_with(".localhost") || value.ends_with(".local") {
        return Err(ApiError::BadRequest(
            "private monitor hosts are not allowed",
        ));
    }
    if let Ok(address) = value.parse::<IpAddr>()
        && (address.is_loopback()
            || address.is_unspecified()
            || address.is_multicast()
            || is_private_ip(address))
    {
        return Err(ApiError::BadRequest(
            "private monitor hosts are not allowed",
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
