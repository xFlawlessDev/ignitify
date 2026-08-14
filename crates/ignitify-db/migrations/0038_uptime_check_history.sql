CREATE TABLE uptime_monitor_checks (
    id TEXT PRIMARY KEY,
    monitor_id TEXT NOT NULL REFERENCES uptime_monitors(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('up', 'down')),
    latency_ms INTEGER,
    error TEXT,
    checked_at TEXT NOT NULL
);

CREATE INDEX uptime_monitor_checks_monitor_checked_idx
    ON uptime_monitor_checks(monitor_id, checked_at DESC);
