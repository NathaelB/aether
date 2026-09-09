-- Placement counted deployments: a freemium instance on 1Gi and an enterprise
-- one on 100Gi cost a data plane exactly the same. The binding constraint on a
-- cluster that runs an IAM workload and a Postgres cluster per deployment is
-- CPU, memory and disk.
--
-- Existing rows are converted rather than dropped. The old `capacity` was a
-- number of deployments, so it is multiplied by the default size a deployment
-- now declares -- which keeps every data plane holding what it held before,
-- expressed in the new unit.
ALTER TABLE data_planes
    ADD COLUMN capacity_cpu_millis INTEGER,
    ADD COLUMN capacity_memory_mib INTEGER,
    ADD COLUMN capacity_storage_gib INTEGER;

UPDATE data_planes SET
    capacity_cpu_millis  = capacity * 500,
    capacity_memory_mib  = capacity * 1024,
    capacity_storage_gib = capacity * 1;

ALTER TABLE data_planes
    ALTER COLUMN capacity_cpu_millis SET NOT NULL,
    ALTER COLUMN capacity_memory_mib SET NOT NULL,
    ALTER COLUMN capacity_storage_gib SET NOT NULL,
    DROP COLUMN capacity;

-- A deployment now states what it costs. Genesis used to invent this at the
-- other end of the chain, so the number that reserved room and the number
-- written into the IdentityInstance were unrelated.
ALTER TABLE deployments
    ADD COLUMN cpu_millis INTEGER NOT NULL DEFAULT 500,
    ADD COLUMN memory_mib INTEGER NOT NULL DEFAULT 1024,
    ADD COLUMN storage_gib INTEGER NOT NULL DEFAULT 1;

-- The defaults exist only to convert existing rows; new ones always carry an
-- explicit size, so leaving them in place would hide a missing value.
ALTER TABLE deployments
    ALTER COLUMN cpu_millis DROP DEFAULT,
    ALTER COLUMN memory_mib DROP DEFAULT,
    ALTER COLUMN storage_gib DROP DEFAULT;
