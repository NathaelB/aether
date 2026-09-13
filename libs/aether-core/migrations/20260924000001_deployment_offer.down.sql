ALTER TABLE deployments
    DROP CONSTRAINT IF EXISTS deployments_environment_known;

ALTER TABLE deployments
    DROP COLUMN IF EXISTS offer,
    DROP COLUMN IF EXISTS environment;
