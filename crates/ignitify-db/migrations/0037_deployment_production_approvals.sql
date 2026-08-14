ALTER TABLE deployments ADD COLUMN approval_status TEXT NOT NULL DEFAULT 'not_required'
    CHECK (approval_status IN ('not_required', 'pending', 'approved'));
ALTER TABLE deployments ADD COLUMN approval_requested_at TEXT;
ALTER TABLE deployments ADD COLUMN approved_by_user_id TEXT REFERENCES users(id) ON DELETE RESTRICT;
ALTER TABLE deployments ADD COLUMN approved_at TEXT;

CREATE INDEX idx_deployments_pending_approval
    ON deployments(created_at)
    WHERE approval_status = 'pending';
