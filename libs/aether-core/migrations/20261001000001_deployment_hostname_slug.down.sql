DROP INDEX IF EXISTS idx_deployments_hostname_slug;
ALTER TABLE deployments DROP COLUMN IF EXISTS hostname_slug;
