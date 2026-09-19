-- What a deployment's own hostname is derived from (slug(name), #282),
-- stored rather than recomputed by SQL: the rule lives in Rust
-- (deployments::environment::slug) and this column exists so a real
-- constraint can be put on it, not so it can be read back.
--
-- Backfilled with a best-effort approximation of slug() for existing rows.
-- Good enough for a one-time migration on data nothing production-critical
-- has ever collided on; the column is kept in sync by the application from
-- here on, on every write that touches name (create, rename, cutover).
ALTER TABLE deployments ADD COLUMN hostname_slug VARCHAR(255);

UPDATE deployments
SET hostname_slug = trim(both '-' from lower(regexp_replace(name, '[^a-zA-Z0-9]+', '-', 'g')));

ALTER TABLE deployments ALTER COLUMN hostname_slug SET NOT NULL;

-- Two live deployments in one organisation cannot hold the same hostname --
-- proven here, not only in the service. Partial, not the deployment's own
-- name: a deleted deployment's slug does not block a new one from reusing
-- it, the same way `idx_deployments_active` already scopes liveness.
--
-- Per organisation, not globally: #281 scopes a deployment's hostname by
-- its organisation's own slug, which is unique by construction, so two
-- organisations naming a deployment alike were never going to collide in
-- the first place.
CREATE UNIQUE INDEX idx_deployments_hostname_slug
    ON deployments (organisation_id, hostname_slug)
    WHERE deleted_at IS NULL;
