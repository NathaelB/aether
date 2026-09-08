DROP INDEX IF EXISTS idx_data_planes_shared_active;

ALTER TABLE data_planes DROP COLUMN last_seen_at;

CREATE INDEX idx_data_planes_shared_active
ON data_planes (region)
WHERE mode = 'shared' AND status = 'active';
