ALTER TABLE notification_deliveries
    ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0);

UPDATE notification_deliveries
SET attempt_count = 1
WHERE status IN ('succeeded', 'failed');
