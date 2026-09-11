ALTER TABLE server_settings
ADD COLUMN application_domain_suffixes_json TEXT NOT NULL DEFAULT '[]';
