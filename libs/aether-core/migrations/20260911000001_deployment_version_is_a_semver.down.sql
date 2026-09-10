ALTER TABLE deployments DROP CONSTRAINT IF EXISTS deployments_version_is_semver;
ALTER TABLE deployments ALTER COLUMN version DROP NOT NULL;
