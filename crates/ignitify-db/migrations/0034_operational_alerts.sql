CREATE TABLE operational_alerts (
    alert_key TEXT PRIMARY KEY,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    generation INTEGER NOT NULL CHECK (generation >= 1),
    activated_at TEXT NOT NULL,
    resolved_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE operational_alert_events (
    alert_key TEXT NOT NULL REFERENCES operational_alerts(alert_key) ON DELETE CASCADE,
    generation INTEGER NOT NULL CHECK (generation >= 1),
    kind TEXT NOT NULL CHECK (kind IN ('raised', 'resolved')),
    created_at TEXT NOT NULL,
    dispatched_at TEXT,
    PRIMARY KEY (alert_key, generation, kind)
);

CREATE INDEX operational_alert_events_pending_idx
    ON operational_alert_events(dispatched_at, created_at, alert_key);

CREATE TABLE notification_deliveries_next (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('deployment', 'backup', 'remote', 'operations')),
    source_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    created_at TEXT NOT NULL,
    completed_at TEXT,
    message TEXT,
    UNIQUE(channel_id, source_kind, source_id, event_kind)
);

INSERT INTO notification_deliveries_next
    (id, channel_id, source_kind, source_id, event_kind, status, attempt_count,
     created_at, completed_at, message)
SELECT id, channel_id, source_kind, source_id, event_kind, status, attempt_count,
       created_at, completed_at, message
FROM notification_deliveries;

DROP TABLE notification_deliveries;
ALTER TABLE notification_deliveries_next RENAME TO notification_deliveries;

CREATE INDEX notification_deliveries_created_idx ON notification_deliveries(created_at DESC);
