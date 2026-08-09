ALTER TABLE server_settings
ADD COLUMN fallback_page_heading TEXT NOT NULL DEFAULT 'Application not found'
CHECK (length(fallback_page_heading) BETWEEN 1 AND 100);

ALTER TABLE server_settings
ADD COLUMN fallback_page_message TEXT NOT NULL DEFAULT 'The requested hostname is not connected to an active application.'
CHECK (length(fallback_page_message) BETWEEN 1 AND 280);
