-- Add up migration script here
CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    permissions BIGINT NOT NULL DEFAULT 0,
    organisation_id UUID REFERENCES organisations(id) ON DELETE CASCADE,
    color VARCHAR(7),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_roles_organisation_id ON roles(organisation_id);

CREATE INDEX idx_roles_global ON roles(organisation_id) WHERE organisation_id IS NULL;

CREATE UNIQUE INDEX idx_roles_name_org ON roles(name, organisation_id);

CREATE TABLE IF NOT EXISTS member_roles (
    member_id UUID REFERENCES members(id) ON DELETE CASCADE,
    role_id UUID REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (member_id, role_id)
);

CREATE INDEX idx_member_roles_role_id ON member_roles(role_id);

-- Create index on member_id for faster user permission checks
CREATE INDEX idx_member_roles_member_id ON member_roles(member_id);
