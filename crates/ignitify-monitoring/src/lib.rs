use std::{
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use ignitify_db::{Database, UptimeCheckUpdate, UptimeMonitorRecord};
use reqwest::{Client, StatusCode, redirect::Policy};
use tokio::{net::TcpStream, task::JoinHandle, time::timeout};
use url::{Host, Url};

const POLL_INTERVAL: Duration = Duration::from_secs(15);
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const AGENT_HEARTBEAT_TIMEOUT: chrono::Duration = chrono::Duration::seconds(90);

#[derive(Debug, Clone)]
pub struct MonitorWorker {
    database: Database,
}

impl MonitorWorker {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move { self.run().await })
    }

    async fn run(self) {
        let client = match Client::builder()
            .timeout(CHECK_TIMEOUT)
            .redirect(Policy::none())
            .user_agent("Ignitify-Uptime-Monitor/0.1")
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                tracing::error!(%error, "uptime monitor HTTP client could not be initialized");
                return;
            }
        };
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            if let Err(error) = self.check_due(&client).await {
                tracing::error!(%error, "uptime monitor cycle failed");
            }
        }
    }

    async fn check_due(&self, client: &Client) -> Result<(), ignitify_db::DatabaseError> {
        let agent_cutoff = (Utc::now() - AGENT_HEARTBEAT_TIMEOUT).to_rfc3339();
        self.database
            .remote_server_agents()
            .mark_stale(&agent_cutoff)
            .await?;
        let monitors = self.database.uptime_monitors().list_enabled().await?;
        for monitor in monitors {
            if !is_due(&monitor, Utc::now()) {
                continue;
            }
            let expected_updated_at = monitor.updated_at.clone();
            let result = check_monitor(client, &monitor).await;
            let update = match result {
                Ok(check) => UptimeCheckUpdate {
                    status: "up".to_owned(),
                    latency_ms: Some(check.latency_ms),
                    last_error: None,
                    checked_at: check.checked_at,
                },
                Err(check) => UptimeCheckUpdate {
                    status: "down".to_owned(),
                    latency_ms: Some(check.latency_ms),
                    last_error: Some(check.error),
                    checked_at: check.checked_at,
                },
            };
            self.database
                .uptime_monitors()
                .record_check(&monitor.id, &expected_updated_at, update)
                .await?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct CheckSuccess {
    latency_ms: u64,
    checked_at: String,
}

#[derive(Debug)]
struct CheckFailure {
    latency_ms: u64,
    error: String,
    checked_at: String,
}

async fn check_monitor(
    client: &Client,
    monitor: &UptimeMonitorRecord,
) -> Result<CheckSuccess, CheckFailure> {
    let started = Instant::now();
    let checked_at = Utc::now().to_rfc3339();
    match monitor.kind.as_str() {
        "http" => {
            if let Err(error) = ensure_public_http_target(&monitor.target).await {
                return Err(CheckFailure {
                    latency_ms: elapsed_millis(started),
                    error,
                    checked_at,
                });
            }
            let result = client.get(&monitor.target).send().await;
            let latency_ms = elapsed_millis(started);
            match result {
                Ok(response) if response.status().is_success() => Ok(CheckSuccess {
                    latency_ms,
                    checked_at,
                }),
                Ok(response) => Err(CheckFailure {
                    latency_ms,
                    error: format_http_error(response.status()),
                    checked_at,
                }),
                Err(error) => Err(CheckFailure {
                    latency_ms,
                    error: safe_request_error(&error),
                    checked_at,
                }),
            }
        }
        "tcp" => {
            let (host, port) = match parse_tcp_target(&monitor.target) {
                Some(target) => target,
                None => {
                    return Err(CheckFailure {
                        latency_ms: elapsed_millis(started),
                        error: "TCP target is invalid".to_owned(),
                        checked_at,
                    });
                }
            };
            let addresses = match tokio::net::lookup_host((host.as_str(), port)).await {
                Ok(addresses) => addresses.collect::<Vec<_>>(),
                Err(_) => {
                    return Err(CheckFailure {
                        latency_ms: elapsed_millis(started),
                        error: "DNS lookup failed".to_owned(),
                        checked_at,
                    });
                }
            };
            if addresses.is_empty() || addresses.iter().any(|address| is_private_ip(address.ip())) {
                return Err(CheckFailure {
                    latency_ms: elapsed_millis(started),
                    error: "target resolved to a private address".to_owned(),
                    checked_at,
                });
            }
            match timeout(CHECK_TIMEOUT, connect_any(addresses)).await {
                Ok(Ok(())) => Ok(CheckSuccess {
                    latency_ms: elapsed_millis(started),
                    checked_at,
                }),
                Ok(Err(_)) | Err(_) => Err(CheckFailure {
                    latency_ms: elapsed_millis(started),
                    error: "TCP connection failed".to_owned(),
                    checked_at,
                }),
            }
        }
        _ => Err(CheckFailure {
            latency_ms: elapsed_millis(started),
            error: "monitor type is invalid".to_owned(),
            checked_at,
        }),
    }
}

async fn ensure_public_http_target(target: &str) -> Result<(), String> {
    let url = Url::parse(target).map_err(|_| "HTTP target is invalid".to_owned())?;
    let host = url
        .host_str()
        .ok_or_else(|| "HTTP target has no hostname".to_owned())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "HTTP target has no port".to_owned())?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "DNS lookup failed".to_owned())?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| is_private_ip(address.ip())) {
        return Err("target resolved to a private address".to_owned());
    }
    Ok(())
}

async fn connect_any(addresses: Vec<SocketAddr>) -> std::io::Result<()> {
    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect(address).await {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("no addresses")))
}

fn is_due(monitor: &UptimeMonitorRecord, now: DateTime<Utc>) -> bool {
    let Some(last_checked_at) = &monitor.last_checked_at else {
        return true;
    };
    let Ok(last_checked_at) = DateTime::parse_from_rfc3339(last_checked_at) else {
        return true;
    };
    now.signed_duration_since(last_checked_at.with_timezone(&Utc))
        >= chrono::Duration::seconds(monitor.interval_seconds)
}

fn parse_tcp_target(target: &str) -> Option<(String, u16)> {
    let (host, port) = target.rsplit_once(':')?;
    let port = port.parse().ok()?;
    if host.is_empty() || host.contains(':') || Host::parse(host).is_err() {
        return None;
    }
    Some((host.to_owned(), port))
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn format_http_error(status: StatusCode) -> String {
    format!("HTTP status {}", status.as_u16())
}

fn safe_request_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "request timed out".to_owned()
    } else if error.is_connect() {
        "connection failed".to_owned()
    } else {
        "request failed".to_owned()
    }
}

fn is_private_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => {
            value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_unspecified()
                || value.is_multicast()
        }
        IpAddr::V6(value) => {
            value.is_unique_local()
                || value.is_loopback()
                || value.is_unicast_link_local()
                || value.is_unspecified()
                || value.is_multicast()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_due, parse_tcp_target};
    use chrono::{Duration, Utc};
    use ignitify_db::UptimeMonitorRecord;

    fn monitor(last_checked_at: Option<String>) -> UptimeMonitorRecord {
        UptimeMonitorRecord {
            id: "monitor".to_owned(),
            user_id: "user".to_owned(),
            name: "Monitor".to_owned(),
            target: "https://example.com".to_owned(),
            kind: "http".to_owned(),
            interval_seconds: 60,
            enabled: true,
            status: "pending".to_owned(),
            history: vec![],
            latency_ms: None,
            last_checked_at,
            last_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn parses_tcp_monitor_target() {
        assert_eq!(
            parse_tcp_target("cache.example.com:6379"),
            Some(("cache.example.com".to_owned(), 6379))
        );
        assert!(parse_tcp_target("cache.example.com").is_none());
    }

    #[test]
    fn schedules_first_check_and_respects_interval() {
        let now = Utc::now();
        assert!(is_due(&monitor(None), now));
        assert!(!is_due(
            &monitor(Some((now - Duration::seconds(30)).to_rfc3339())),
            now
        ));
        assert!(is_due(
            &monitor(Some((now - Duration::seconds(60)).to_rfc3339())),
            now
        ));
    }
}
