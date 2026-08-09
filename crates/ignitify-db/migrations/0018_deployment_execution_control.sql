ALTER TABLE deployments
ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0
CHECK (attempt_count >= 0);

ALTER TABLE deployments
ADD COLUMN retry_after TEXT;

ALTER TABLE deployments
ADD COLUMN cancel_requested_at TEXT;

CREATE INDEX idx_deployments_queue_ready
ON deployments(status, retry_after, created_at);
