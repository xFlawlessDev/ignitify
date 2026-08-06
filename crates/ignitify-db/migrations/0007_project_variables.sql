CREATE TABLE project_variables (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    is_secret INTEGER NOT NULL CHECK (is_secret IN (0, 1)),
    ciphertext TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (project_id, key)
);

CREATE INDEX idx_project_variables_project_key ON project_variables(project_id, key);
