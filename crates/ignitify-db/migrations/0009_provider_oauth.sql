CREATE TABLE providers_v2 (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('git', 'gitea', 'gitlab', 'github')),
    auth_mode TEXT NOT NULL CHECK (auth_mode IN ('token', 'oauth', 'github_app')),
    base_url TEXT NOT NULL,
    internal_url TEXT,
    redirect_uri TEXT,
    client_id TEXT,
    application_id TEXT,
    installation_id TEXT,
    group_names TEXT,
    username TEXT,
    token_ciphertext TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_verified_at TEXT
);

INSERT INTO providers_v2 (
    id, name, kind, auth_mode, base_url, username, token_ciphertext,
    created_by, created_at, updated_at, last_verified_at
)
SELECT
    id, name, kind, 'token', base_url, username, token_ciphertext,
    created_by, created_at, updated_at, last_verified_at
FROM providers;

DROP TABLE providers;
ALTER TABLE providers_v2 RENAME TO providers;

CREATE UNIQUE INDEX idx_providers_name ON providers(name COLLATE NOCASE);
CREATE INDEX idx_providers_updated ON providers(updated_at DESC);
