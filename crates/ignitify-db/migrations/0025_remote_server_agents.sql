CREATE TABLE IF NOT EXISTS remote_server_agents (
    server_id TEXT PRIMARY KEY REFERENCES remote_servers(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'online', 'offline')),
    version TEXT,
    cpu_usage_percentage REAL,
    cpu_cores INTEGER,
    memory_used_bytes INTEGER,
    memory_total_bytes INTEGER,
    disk_used_bytes INTEGER,
    disk_total_bytes INTEGER,
    docker_containers INTEGER,
    docker_running_containers INTEGER,
    last_heartbeat_at TEXT,
    last_error TEXT,
    installed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS remote_server_agent_events (
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES remote_servers(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('provisioned', 'online', 'offline', 'error')),
    message TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_remote_server_agent_events_server
    ON remote_server_agent_events(server_id, created_at DESC);
