CREATE TABLE deployments (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    generation INTEGER NOT NULL,
    idempotency_key TEXT NOT NULL,
    requested_by_user_id TEXT NOT NULL REFERENCES users(id),
    spec_json TEXT NOT NULL,
    variables_ciphertext TEXT NOT NULL,
    runtime_ref TEXT,
    status TEXT NOT NULL CHECK (status IN ('queued', 'preparing', 'running', 'healthy', 'failed', 'stopping', 'stopped', 'superseded')),
    failure_reason TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    UNIQUE (service_id, generation),
    UNIQUE (service_id, idempotency_key)
);

CREATE TABLE deployment_events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE deployment_logs (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    stream TEXT NOT NULL CHECK (stream IN ('stdout', 'stderr', 'system')),
    line TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_deployments_service_created ON deployments(service_id, created_at DESC);
CREATE INDEX idx_deployments_status_created ON deployments(status, created_at);
CREATE UNIQUE INDEX idx_deployments_active_service ON deployments(service_id)
    WHERE status IN ('queued', 'preparing', 'running');
CREATE INDEX idx_deployment_events_deployment_sequence ON deployment_events(deployment_id, sequence);
CREATE INDEX idx_deployment_logs_deployment_sequence ON deployment_logs(deployment_id, sequence);
