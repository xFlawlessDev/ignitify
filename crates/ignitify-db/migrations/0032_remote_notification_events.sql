CREATE TABLE remote_notification_events (
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES remote_servers(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('remote_agent.offline', 'remote_server.authentication_failed')),
    message TEXT NOT NULL,
    created_at TEXT NOT NULL,
    dispatched_at TEXT
);

CREATE INDEX remote_notification_events_pending_idx
    ON remote_notification_events(dispatched_at, created_at, id);

CREATE TABLE remote_server_authentication_failures (
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES remote_servers(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

CREATE INDEX remote_server_authentication_failures_server_created_idx
    ON remote_server_authentication_failures(server_id, created_at DESC);

CREATE TABLE notification_deliveries_next (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('deployment', 'backup', 'remote')),
    source_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    created_at TEXT NOT NULL,
    completed_at TEXT,
    message TEXT,
    UNIQUE(channel_id, source_kind, source_id, event_kind)
);

INSERT INTO notification_deliveries_next
    (id, channel_id, source_kind, source_id, event_kind, status, created_at, completed_at, message)
SELECT id, channel_id, source_kind, source_id, event_kind, status, created_at, completed_at, message
FROM notification_deliveries;

DROP TABLE notification_deliveries;
ALTER TABLE notification_deliveries_next RENAME TO notification_deliveries;

CREATE INDEX notification_deliveries_created_idx ON notification_deliveries(created_at DESC);
