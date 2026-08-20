ALTER TABLE deployments
    ADD COLUMN rollback_of_deployment_id TEXT REFERENCES deployments(id) ON DELETE SET NULL;

CREATE INDEX idx_deployments_rollback_of
    ON deployments(rollback_of_deployment_id);
