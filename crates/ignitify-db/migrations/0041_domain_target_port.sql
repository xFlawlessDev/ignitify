ALTER TABLE domains
    ADD COLUMN target_port INTEGER NOT NULL DEFAULT 80
    CHECK (target_port BETWEEN 1 AND 65535);

UPDATE domains
SET target_port = COALESCE(
    (
        SELECT CAST(json_extract(services.desired_spec_json, '$.internal_port') AS INTEGER)
        FROM services
        WHERE services.id = domains.service_id
    ),
    target_port
);
