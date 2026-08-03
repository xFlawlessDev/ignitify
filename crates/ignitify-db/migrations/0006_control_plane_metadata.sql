CREATE TABLE registries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    endpoint TEXT NOT NULL,
    username TEXT,
    credential_ciphertext TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE project_webhooks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    secret_ciphertext TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (project_id, name)
);

CREATE INDEX idx_project_webhooks_project ON project_webhooks(project_id, updated_at DESC);
CREATE INDEX idx_audit_logs_resource_created ON audit_logs(resource_type, resource_id, created_at DESC);
