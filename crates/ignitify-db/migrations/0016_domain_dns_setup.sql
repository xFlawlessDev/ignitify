ALTER TABLE server_settings
ADD COLUMN dns_record_type TEXT NOT NULL DEFAULT 'a'
CHECK (dns_record_type IN ('a', 'cname'));

ALTER TABLE server_settings
ADD COLUMN dns_record_target TEXT NOT NULL DEFAULT '';

ALTER TABLE domains
ADD COLUMN dns_record_type TEXT NOT NULL DEFAULT 'a'
CHECK (dns_record_type IN ('a', 'cname'));

ALTER TABLE domains
ADD COLUMN dns_record_target TEXT NOT NULL DEFAULT '';

ALTER TABLE domains
ADD COLUMN dns_status TEXT NOT NULL DEFAULT 'not_checked'
CHECK (dns_status IN ('not_checked', 'pending', 'valid', 'missing', 'unavailable'));

ALTER TABLE domains
ADD COLUMN dns_error TEXT;

ALTER TABLE domains
ADD COLUMN dns_checked_at TEXT;

ALTER TABLE domains
ADD COLUMN dns_verification_requested_at TEXT;
