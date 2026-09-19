-- What the last drill actually proved (#185) -- not claimed, measured: the
-- last time an automated restore of this deployment's own archive was
-- carried out end to end and answered a query, and how long it took.
--
-- Both nullable, and stay nullable: a deployment nothing has drilled yet, or
-- one whose only drill failed, looks exactly like one created before this
-- shipped. A failed drill leaves the previous successful timestamp standing
-- rather than clearing it -- "last verified" means the last time it actually
-- worked, not the last time somebody tried.
ALTER TABLE deployments ADD COLUMN last_verified_restore_at TIMESTAMPTZ;
ALTER TABLE deployments ADD COLUMN last_restore_drill_seconds INTEGER;
