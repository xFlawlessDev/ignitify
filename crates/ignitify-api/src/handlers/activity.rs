use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use ignitify_db::{ActivityActor, ActivityRecord};
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, extract::require_actor, state::AppState};

#[derive(Debug, Deserialize)]
pub(crate) struct ActivityQuery {
    before: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActivityResponse {
    id: String,
    action: String,
    resource_type: Option<String>,
    resource_id: Option<String>,
    created_at: String,
}

impl From<ActivityRecord> for ActivityResponse {
    fn from(record: ActivityRecord) -> Self {
        Self {
            id: record.id,
            action: record.action,
            resource_type: record.resource_type,
            resource_id: record.resource_id,
            created_at: record.created_at,
        }
    }
}

pub(crate) async fn list_for_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<Vec<ActivityResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let activity = state
        .database
        .activity()
        .list_for_project(
            ActivityActor {
                id: &actor.id,
                is_admin: actor.has_platform_operator_access(),
            },
            &project_id,
            query.before.as_deref(),
            query.limit,
        )
        .await?
        .ok_or(ApiError::NotFound)?
        .into_iter()
        .map(ActivityResponse::from)
        .collect();
    Ok(Json(activity))
}
