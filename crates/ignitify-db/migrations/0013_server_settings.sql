CREATE TABLE server_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    server_domain TEXT NOT NULL DEFAULT '',
    https_enabled INTEGER NOT NULL DEFAULT 0 CHECK (https_enabled IN (0, 1)),
    automatically_provision_ssl INTEGER NOT NULL DEFAULT 0 CHECK (automatically_provision_ssl IN (0, 1)),
    certificate_provider TEXT NOT NULL DEFAULT 'none'
        CHECK (certificate_provider IN ('none', 'lets-encrypt', 'custom')),
    custom_certificate_id TEXT,
    concurrent_builds INTEGER NOT NULL DEFAULT 2 CHECK (concurrent_builds BETWEEN 1 AND 32),
    updated_at TEXT NOT NULL
);

INSERT INTO server_settings (id, updated_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TABLE server_certificates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    certificate_file_name TEXT NOT NULL,
    private_key_file_name TEXT NOT NULL,
    certificate_ciphertext TEXT NOT NULL,
    private_key_ciphertext TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_server_certificates_name ON server_certificates(name COLLATE NOCASE);
