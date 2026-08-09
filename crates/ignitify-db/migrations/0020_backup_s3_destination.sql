CREATE TABLE backup_s3_destination (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    endpoint TEXT NOT NULL,
    region TEXT NOT NULL,
    bucket TEXT NOT NULL,
    prefix TEXT NOT NULL,
    access_key_id_ciphertext TEXT NOT NULL,
    secret_access_key_ciphertext TEXT NOT NULL,
    session_token_ciphertext TEXT,
    server_side_encryption TEXT NOT NULL DEFAULT 'AES256'
        CHECK (server_side_encryption IN ('provider-default', 'AES256')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
