-- Add down migration script here
DROP INDEX IF EXISTS idx_deployments_active;
DROP INDEX IF EXISTS idx_deployments_status;
DROP INDEX IF EXISTS idx_deployments_created_by;
DROP INDEX IF EXISTS idx_deployments_organisation_id;
DROP INDEX IF EXISTS idx_deployments_dataplane;

DROP TABLE IF EXISTS deployments;
