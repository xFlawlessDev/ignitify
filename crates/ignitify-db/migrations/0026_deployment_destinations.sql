ALTER TABLE services ADD COLUMN deployment_destination_id TEXT REFERENCES remote_servers(id) ON DELETE RESTRICT;
ALTER TABLE deployments ADD COLUMN deployment_destination_id TEXT REFERENCES remote_servers(id) ON DELETE RESTRICT;

CREATE INDEX idx_services_deployment_destination
    ON services(deployment_destination_id);

CREATE INDEX idx_deployments_deployment_destination
    ON deployments(deployment_destination_id);
