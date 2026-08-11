CREATE TABLE notification_channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('telegram', 'discord', 'smtp', 'resend', 'webhook')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    event_types_json TEXT NOT NULL,
    configuration_summary_json TEXT NOT NULL,
    configuration_ciphertext TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX notification_channels_enabled_idx ON notification_channels (enabled);

CREATE TABLE notification_deliveries (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('deployment', 'backup')),
    source_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    created_at TEXT NOT NULL,
    completed_at TEXT,
    message TEXT,
    UNIQUE(channel_id, source_kind, source_id, event_kind)
);

CREATE INDEX notification_deliveries_created_idx ON notification_deliveries (created_at DESC);
