ALTER TABLE deployments DROP CONSTRAINT IF EXISTS deployments_allow_list_is_never_empty;
ALTER TABLE deployments DROP COLUMN IF EXISTS allowed_cidrs;
