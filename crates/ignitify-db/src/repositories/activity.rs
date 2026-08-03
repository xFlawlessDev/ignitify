use sqlx::{FromRow, SqlitePool};

use crate::Result;

const ACTIVITY_PAGE_SIZE: i64 = 50;
const ACTIVITY_PAGE_MAX: i64 = 100;

#[derive(Debug, Clone, Copy)]
pub struct ActivityActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct ActivityRecord {
    pub id: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ActivityRepository {
    pool: SqlitePool,
}

impl ActivityRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_for_project(
        &self,
        actor: ActivityActor<'_>,
        project_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<ActivityRecord>>> {
        if !self.can_view_project(actor, project_id).await? {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, ActivityRow>(
            "SELECT a.id, a.action, a.resource_type, a.resource_id, a.created_at
             FROM audit_logs a
             WHERE (
                (a.resource_type = 'project' AND a.resource_id = ?)
             OR (a.resource_type = 'service' AND a.resource_id IN (
                    SELECT s.id FROM services s
                    JOIN environments e ON e.id = s.environment_id
                    WHERE e.project_id = ?
             )) OR (a.resource_type = 'deployment' AND a.resource_id IN (
                    SELECT d.id FROM deployments d
                    JOIN services s ON s.id = d.service_id
                    JOIN environments e ON e.id = s.environment_id
                    WHERE e.project_id = ?
             )) OR (a.resource_type = 'domain' AND a.resource_id IN (
                    SELECT dm.id FROM domains dm
                    JOIN services s ON s.id = dm.service_id
                    JOIN environments e ON e.id = s.environment_id
                    WHERE e.project_id = ?
             )) OR (a.resource_type = 'webhook' AND a.resource_id IN (
                    SELECT w.id FROM project_webhooks w WHERE w.project_id = ?
             )))
             AND (? IS NULL OR a.created_at < ?)
             ORDER BY a.created_at DESC
             LIMIT ?",
        )
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(project_id)
        .bind(before_created_at)
        .bind(before_created_at)
        .bind(page_limit(limit))
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(
            rows.into_iter()
                .map(|row| ActivityRecord {
                    id: row.id,
                    action: row.action,
                    resource_type: row.resource_type,
                    resource_id: row.resource_id,
                    created_at: row.created_at,
                })
                .collect(),
        ))
    }

    async fn can_view_project(&self, actor: ActivityActor<'_>, project_id: &str) -> Result<bool> {
        if actor.is_admin {
            return Ok(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = ?")
                    .bind(project_id)
                    .fetch_one(&self.pool)
                    .await?
                    > 0,
            );
        }
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM project_members WHERE project_id = ? AND user_id = ?",
        )
        .bind(project_id)
        .bind(actor.id)
        .fetch_one(&self.pool)
        .await?
            > 0)
    }
}

fn page_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(ACTIVITY_PAGE_SIZE)
        .clamp(1, ACTIVITY_PAGE_MAX)
}

#[derive(Debug, FromRow)]
struct ActivityRow {
    id: String,
    action: String,
    resource_type: Option<String>,
    resource_id: Option<String>,
    created_at: String,
}
