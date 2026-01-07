-- Create organisation_members table for tracking organisation memberships
CREATE TABLE IF NOT EXISTS organisation_members (
    organisation_id UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organisation_id, user_id)
);

-- Create index on user_id for faster lookups
CREATE INDEX idx_organisation_members_user_id ON organisation_members(user_id);
