CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE,
    owner_id TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE project_members (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'editor', 'viewer')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (project_id, user_id)
);

CREATE TABLE environments (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (project_id, name COLLATE NOCASE)
);

CREATE INDEX idx_projects_owner_updated ON projects(owner_id, updated_at DESC);
CREATE UNIQUE INDEX idx_projects_owner_name ON projects(owner_id, name);
CREATE INDEX idx_project_members_user_project ON project_members(user_id, project_id);
CREATE INDEX idx_environments_project_default ON environments(project_id, is_default);
CREATE UNIQUE INDEX idx_environments_one_default
    ON environments(project_id)
    WHERE is_default = 1;
