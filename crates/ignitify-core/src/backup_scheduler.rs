use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Duration as ChronoDuration, Utc};

use crate::operations;

const POLL_INTERVAL: Duration = Duration::from_secs(60);

pub(super) fn spawn(
    database: ignitify_db::Database,
    data_dir: PathBuf,
    database_config: ignitify_db::DatabaseConfig,
    secrets_age_identity: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(error) = run_if_due(
                &database,
                &data_dir,
                &database_config,
                &secrets_age_identity,
            )
            .await
            {
                eprintln!("scheduled backup check failed: {error}");
            }
        }
    })
}

async fn run_if_due(
    database: &ignitify_db::Database,
    data_dir: &std::path::Path,
    database_config: &ignitify_db::DatabaseConfig,
    secrets_age_identity: &str,
) -> Result<(), ignitify_db::DatabaseError> {
    let Some(destination) = database.backup_destinations().s3().await? else {
        return Ok(());
    };
    let Some(interval_hours) = destination.schedule_interval_hours else {
        return Ok(());
    };
    if !destination.enabled || !is_due(&destination, database, interval_hours).await? {
        return Ok(());
    }

    let output = data_dir
        .join("backups")
        .join(format!("scheduled-{}", Utc::now().format("%Y%m%dT%H%M%SZ")));
    if let Err(error) =
        operations::scheduled_backup(&output, data_dir, database_config, secrets_age_identity).await
    {
        eprintln!("scheduled backup failed: {error}");
    }
    Ok(())
}

async fn is_due(
    destination: &ignitify_db::BackupS3DestinationRecord,
    database: &ignitify_db::Database,
    interval_hours: u16,
) -> Result<bool, ignitify_db::DatabaseError> {
    let latest_run = database
        .backup_destinations()
        .latest_scheduled_s3_run()
        .await?;
    let since = latest_run
        .as_ref()
        .map(|run| run.started_at.as_str())
        .unwrap_or(&destination.updated_at);
    Ok(is_due_at(Utc::now(), since, interval_hours))
}

fn is_due_at(now: DateTime<Utc>, since: &str, interval_hours: u16) -> bool {
    let Ok(since) = DateTime::parse_from_rfc3339(since) else {
        return false;
    };
    now >= since.with_timezone(&Utc) + ChronoDuration::hours(i64::from(interval_hours))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::is_due_at;

    #[test]
    fn elapsed_interval_is_due() {
        let since = "2026-01-01T00:00:00Z";
        let now = DateTime::parse_from_rfc3339("2026-01-02T00:00:01Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(is_due_at(now, since, 24));
        assert!(!is_due_at(now, since, 25));
        assert!(!is_due_at(now, "not-a-timestamp", 1));
    }
}
