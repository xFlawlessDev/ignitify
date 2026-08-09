use std::{
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};

use crate::{
    ActivityRepository, BackupDestinationsRepository, DashboardRepository, DeploymentsRepository,
    DomainsRepository, EnvironmentsRepository, ProjectsRepository, ProvidersRepository,
    RefreshTokensRepository, RemoteBuildersRepository, RemoteServersRepository, Result,
    ServerSettingsRepository, ServicesRepository, UptimeMonitorsRepository, UsersRepository,
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

impl DatabaseConfig {
    pub fn file_path(&self) -> Option<PathBuf> {
        sqlite_file_path(&self.url)
    }
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

    pub fn activity(&self) -> ActivityRepository {
        ActivityRepository::new(self.pool.clone())
    }

    pub fn backup_destinations(&self) -> BackupDestinationsRepository {
        BackupDestinationsRepository::new(self.pool.clone())
    }

    pub fn dashboard(&self) -> DashboardRepository {
        DashboardRepository::new(self.pool.clone())
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

    pub fn providers(&self) -> ProvidersRepository {
        ProvidersRepository::new(self.pool.clone())
    }

    pub fn remote_builders(&self) -> RemoteBuildersRepository {
        RemoteBuildersRepository::new(self.pool.clone())
    }

    pub fn remote_servers(&self) -> RemoteServersRepository {
        RemoteServersRepository::new(self.pool.clone())
    }

    pub fn environments(&self) -> EnvironmentsRepository {
        EnvironmentsRepository::new(self.pool.clone())
    }

    pub fn services(&self) -> ServicesRepository {
        ServicesRepository::new(self.pool.clone())
    }

    pub fn deployments(&self) -> DeploymentsRepository {
        DeploymentsRepository::new(self.pool.clone())
    }

    pub fn domains(&self) -> DomainsRepository {
        DomainsRepository::new(self.pool.clone())
    }

    pub fn server_settings(&self) -> ServerSettingsRepository {
        ServerSettingsRepository::new(self.pool.clone())
    }

    pub fn uptime_monitors(&self) -> UptimeMonitorsRepository {
        UptimeMonitorsRepository::new(self.pool.clone())
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn backup_to(&self, destination: &Path) -> Result<()> {
        if destination.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "database backup destination already exists",
            )
            .into());
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let destination = destination.to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "database backup destination is not valid UTF-8",
            )
        })?;
        sqlx::query("VACUUM INTO ?")
            .bind(destination)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn close(self) {
        self.pool.close().await;
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
