UPDATE server_settings
SET
    https_enabled = 1,
    automatically_provision_ssl = 1,
    certificate_provider = 'lets-encrypt',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 1
    AND server_domain = ''
    AND https_enabled = 0
    AND automatically_provision_ssl = 0
    AND certificate_provider = 'none'
    AND custom_certificate_id IS NULL
    AND concurrent_builds = 2;
