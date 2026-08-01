ALTER TABLE audit_logs ADD COLUMN resource_type TEXT;
ALTER TABLE audit_logs ADD COLUMN resource_id TEXT;
ALTER TABLE audit_logs ADD COLUMN details_json TEXT;

CREATE TABLE services (
    id TEXT PRIMARY KEY,
    environment_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE,
    kind TEXT NOT NULL CHECK (kind IN ('image', 'compose')),
    desired_spec_json TEXT NOT NULL,
    desired_generation INTEGER NOT NULL DEFAULT 1,
    desired_state TEXT NOT NULL DEFAULT 'stopped' CHECK (desired_state IN ('running', 'stopped')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (environment_id, name)
);

CREATE TABLE service_variables (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    is_secret INTEGER NOT NULL CHECK (is_secret IN (0, 1)),
    ciphertext TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (service_id, key)
);

CREATE INDEX idx_services_environment_updated ON services(environment_id, updated_at DESC);
CREATE INDEX idx_service_variables_service_key ON service_variables(service_id, key);
