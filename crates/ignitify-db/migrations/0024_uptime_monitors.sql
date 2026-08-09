CREATE TABLE IF NOT EXISTS uptime_monitors (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    target TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('http', 'tcp')),
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds BETWEEN 30 AND 86400),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'up', 'down')),
    history_json TEXT NOT NULL DEFAULT '[]',
    latency_ms INTEGER,
    last_checked_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (user_id, name)
);

CREATE INDEX IF NOT EXISTS idx_uptime_monitors_due
    ON uptime_monitors(enabled, last_checked_at);

CREATE INDEX IF NOT EXISTS idx_uptime_monitors_user
    ON uptime_monitors(user_id, updated_at DESC);
