use std::{path::PathBuf, time::Duration as StdDuration};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};

use crate::{
    EnvironmentsRepository, ProjectsRepository, RefreshTokensRepository, Result, UsersRepository,
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:data/ignitify.db".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    pub(crate) pool: SqlitePool,
}

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self> {
        let path = sqlite_file_path(&config.url);
        if let Some(path) = &path
            && let Some(parent) = path.parent()
        {
            std::fs::create_dir_all(parent)?;
        }

        let is_memory = path.is_none();
        let mut options = SqliteConnectOptions::new()
            .filename(path.unwrap_or_else(|| PathBuf::from(":memory:")))
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(StdDuration::from_millis(5_000));
        if !is_memory {
            options = options
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Full);
        }
        let pool = if is_memory {
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await?
        } else {
            SqlitePool::connect_with(options).await?
        };
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn users(&self) -> UsersRepository {
        UsersRepository::new(self.pool.clone())
    }

    pub fn refresh_tokens(&self) -> RefreshTokensRepository {
        RefreshTokensRepository::new(self.pool.clone())
    }

    pub fn projects(&self) -> ProjectsRepository {
        ProjectsRepository::new(self.pool.clone())
    }

    pub fn environments(&self) -> EnvironmentsRepository {
        EnvironmentsRepository::new(self.pool.clone())
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

fn sqlite_file_path(url: &str) -> Option<PathBuf> {
    let path = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))?;
    if path == ":memory:" {
        None
    } else {
        Some(PathBuf::from(path))
    }
}
