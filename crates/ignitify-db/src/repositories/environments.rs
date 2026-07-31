use sqlx::SqlitePool;

use crate::Result;

#[derive(Debug, Clone)]
pub struct EnvironmentsRepository {
    pool: SqlitePool,
}

impl EnvironmentsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn count_for_project(&self, project_id: &str) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM environments WHERE project_id = ?")
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn default_name_for_project(&self, project_id: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "SELECT name FROM environments WHERE project_id = ? AND is_default = 1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
