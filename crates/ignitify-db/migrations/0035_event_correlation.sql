ALTER TABLE deployments ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';

UPDATE deployments
SET correlation_id = 'legacy/deployment/' || id
WHERE correlation_id = '';

CREATE INDEX deployments_correlation_id_idx ON deployments(correlation_id);

ALTER TABLE deployment_events ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';

UPDATE deployment_events
SET correlation_id = (
    SELECT correlation_id
    FROM deployments
    WHERE deployments.id = deployment_events.deployment_id
)
WHERE correlation_id = '';

CREATE INDEX deployment_events_correlation_id_idx
    ON deployment_events(correlation_id, sequence);

ALTER TABLE deployment_logs ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';

UPDATE deployment_logs
SET correlation_id = (
    SELECT correlation_id
    FROM deployments
    WHERE deployments.id = deployment_logs.deployment_id
)
WHERE correlation_id = '';

CREATE INDEX deployment_logs_correlation_id_idx
    ON deployment_logs(correlation_id, sequence);

ALTER TABLE audit_logs ADD COLUMN correlation_id TEXT;

UPDATE audit_logs
SET correlation_id = (
    SELECT correlation_id
    FROM deployments
    WHERE deployments.id = audit_logs.resource_id
)
WHERE resource_type = 'deployment'
  AND correlation_id IS NULL;

CREATE INDEX audit_logs_correlation_id_idx
    ON audit_logs(correlation_id, created_at DESC);

ALTER TABLE notification_deliveries ADD COLUMN correlation_id TEXT;

UPDATE notification_deliveries
SET correlation_id = (
    SELECT correlation_id
    FROM deployment_events
    WHERE deployment_events.sequence = CAST(notification_deliveries.source_id AS INTEGER)
)
WHERE source_kind = 'deployment'
  AND correlation_id IS NULL
  AND source_id GLOB '[0-9]*';

CREATE INDEX notification_deliveries_correlation_id_idx
    ON notification_deliveries(correlation_id, created_at DESC);
