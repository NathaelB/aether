-- A data plane's Herald reports on every sync cycle. Nothing observed that
-- before, so a cluster that disappeared kept being selected for new
-- deployments while the actions it already held stayed Pending forever.
--
-- Nullable on purpose: a data plane that has never reported is not the same
-- thing as one that reported long ago, and only the second is a failure.
-- Backfilling with NOW() would have claimed every existing row had just been
-- heard from, which is exactly the lie this column exists to prevent.
ALTER TABLE data_planes ADD COLUMN last_seen_at TIMESTAMPTZ;

-- Placement filters on region, mode, status and now liveness, so the partial
-- index carries the column it will be compared against.
DROP INDEX IF EXISTS idx_data_planes_shared_active;

CREATE INDEX idx_data_planes_shared_active
ON data_planes (region, last_seen_at)
WHERE mode = 'shared' AND status = 'active';
