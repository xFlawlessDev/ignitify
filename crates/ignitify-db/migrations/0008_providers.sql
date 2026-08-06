CREATE TABLE providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('git', 'gitea', 'gitlab')),
    base_url TEXT NOT NULL,
    username TEXT,
    token_ciphertext TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_verified_at TEXT
);

CREATE UNIQUE INDEX idx_providers_name ON providers(name COLLATE NOCASE);
CREATE INDEX idx_providers_updated ON providers(updated_at DESC);
