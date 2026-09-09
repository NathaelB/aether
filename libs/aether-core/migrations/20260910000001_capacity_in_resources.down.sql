ALTER TABLE deployments
    DROP COLUMN cpu_millis,
    DROP COLUMN memory_mib,
    DROP COLUMN storage_gib;

ALTER TABLE data_planes ADD COLUMN capacity INTEGER;

-- Back to a deployment count, using the same conversion in reverse. A data
-- plane whose capacity was not a whole number of default-sized deployments
-- rounds down rather than claiming room it does not have.
UPDATE data_planes SET capacity = GREATEST(capacity_storage_gib / 1, 1);

ALTER TABLE data_planes
    ALTER COLUMN capacity SET NOT NULL,
    DROP COLUMN capacity_cpu_millis,
    DROP COLUMN capacity_memory_mib,
    DROP COLUMN capacity_storage_gib;
