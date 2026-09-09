DROP INDEX IF EXISTS idx_data_planes_dedicated_owner;

ALTER TABLE data_planes DROP CONSTRAINT IF EXISTS data_planes_dedicated_has_owner;

ALTER TABLE data_planes DROP COLUMN organisation_id;
