use sqlx::{FromRow, SqlitePool};

use crate::Result;

#[derive(Debug, Clone, Copy)]
pub struct DashboardActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct DashboardProjectRecord {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct DashboardServiceRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub kind: String,
    pub desired_generation: i64,
    pub desired_state: String,
}

#[derive(Debug, Clone)]
pub struct DashboardDeploymentRecord {
    pub id: String,
    pub service_id: String,
    pub generation: i64,
    pub status: String,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DashboardRecords {
    pub projects: Vec<DashboardProjectRecord>,
    pub services: Vec<DashboardServiceRecord>,
    pub deployments: Vec<DashboardDeploymentRecord>,
}

#[derive(Debug, Clone)]
pub struct DashboardRepository {
    pool: SqlitePool,
}

impl DashboardRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn summary(&self, actor: DashboardActor<'_>) -> Result<DashboardRecords> {
        let projects = sqlx::query_as::<_, ProjectRow>(
            "SELECT p.id, p.name
             FROM projects p
             LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
             WHERE ? OR pm.user_id IS NOT NULL
             ORDER BY p.updated_at DESC",
        )
        .bind(actor.id)
        .bind(actor.is_admin)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(DashboardProjectRecord::from)
        .collect();
        let services = sqlx::query_as::<_, ServiceRow>(
            "SELECT s.id, e.project_id, s.name, s.kind, s.desired_generation, s.desired_state
             FROM services s
             JOIN environments e ON e.id = s.environment_id
             LEFT JOIN project_members pm ON pm.project_id = e.project_id AND pm.user_id = ?
             WHERE ? OR pm.user_id IS NOT NULL
             ORDER BY s.updated_at DESC",
        )
        .bind(actor.id)
        .bind(actor.is_admin)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(DashboardServiceRecord::from)
        .collect();
        let deployments = sqlx::query_as::<_, DeploymentRow>(
            "SELECT d.id, d.service_id, d.generation, d.status, d.failure_reason,
                    d.created_at, d.started_at, d.finished_at
             FROM deployments d
             JOIN services s ON s.id = d.service_id
             JOIN environments e ON e.id = s.environment_id
             LEFT JOIN project_members pm ON pm.project_id = e.project_id AND pm.user_id = ?
             WHERE ? OR pm.user_id IS NOT NULL
             ORDER BY d.created_at DESC
             LIMIT 500",
        )
        .bind(actor.id)
        .bind(actor.is_admin)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(DashboardDeploymentRecord::from)
        .collect();
        Ok(DashboardRecords {
            projects,
            services,
            deployments,
        })
    }
}

#[derive(Debug, FromRow)]
struct ProjectRow {
    id: String,
    name: String,
}

impl From<ProjectRow> for DashboardProjectRecord {
    fn from(row: ProjectRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
        }
    }
}

#[derive(Debug, FromRow)]
struct ServiceRow {
    id: String,
    project_id: String,
    name: String,
    kind: String,
    desired_generation: i64,
    desired_state: String,
}

impl From<ServiceRow> for DashboardServiceRecord {
    fn from(row: ServiceRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            kind: row.kind,
            desired_generation: row.desired_generation,
            desired_state: row.desired_state,
        }
    }
}

#[derive(Debug, FromRow)]
struct DeploymentRow {
    id: String,
    service_id: String,
    generation: i64,
    status: String,
    failure_reason: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl From<DeploymentRow> for DashboardDeploymentRecord {
    fn from(row: DeploymentRow) -> Self {
        Self {
            id: row.id,
            service_id: row.service_id,
            generation: row.generation,
            status: row.status,
            failure_reason: row.failure_reason,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}
