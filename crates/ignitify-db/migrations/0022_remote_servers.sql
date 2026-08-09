CREATE TABLE remote_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    username TEXT NOT NULL,
    deploy_path TEXT NOT NULL,
    private_key_ciphertext TEXT NOT NULL,
    known_hosts_ciphertext TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_remote_servers_default
ON remote_servers(is_default)
WHERE is_default = 1;
