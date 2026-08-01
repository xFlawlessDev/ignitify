CREATE TABLE domains (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    hostname TEXT NOT NULL COLLATE NOCASE UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'failed')),
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_domains_service ON domains(service_id);
