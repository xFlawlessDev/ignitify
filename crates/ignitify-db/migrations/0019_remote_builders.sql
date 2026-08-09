CREATE TABLE remote_builders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    endpoint TEXT NOT NULL,
    registry_repository TEXT NOT NULL,
    tls_server_name TEXT,
    ca_certificate_ciphertext TEXT NOT NULL,
    client_certificate_ciphertext TEXT NOT NULL,
    client_key_ciphertext TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_remote_builders_default
ON remote_builders(is_default)
WHERE is_default = 1;
