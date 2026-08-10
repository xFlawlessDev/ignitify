ALTER TABLE backup_s3_destination ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1
    CHECK (enabled IN (0, 1));

ALTER TABLE backup_s3_destination ADD COLUMN schedule_interval_hours INTEGER
    CHECK (schedule_interval_hours IS NULL OR schedule_interval_hours BETWEEN 1 AND 720);

CREATE TABLE backup_s3_run (
    id TEXT PRIMARY KEY,
    trigger TEXT NOT NULL CHECK (trigger IN ('manual', 'scheduled')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    message TEXT
);

CREATE INDEX backup_s3_run_started_at_idx ON backup_s3_run (started_at DESC);
