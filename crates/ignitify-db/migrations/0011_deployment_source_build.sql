ALTER TABLE deployments ADD COLUMN source_config_json TEXT;
ALTER TABLE deployments ADD COLUMN source_revision TEXT;
ALTER TABLE deployments ADD COLUMN local_image_id TEXT;
