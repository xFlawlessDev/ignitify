ALTER TABLE audit_logs ADD COLUMN source_ip TEXT;
ALTER TABLE audit_logs ADD COLUMN session_family_id TEXT;
ALTER TABLE audit_logs ADD COLUMN request_id TEXT;
ALTER TABLE audit_logs ADD COLUMN user_agent TEXT;
ALTER TABLE audit_logs ADD COLUMN outcome TEXT NOT NULL DEFAULT 'success';

CREATE INDEX IF NOT EXISTS idx_audit_logs_request_id ON audit_logs(request_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);
