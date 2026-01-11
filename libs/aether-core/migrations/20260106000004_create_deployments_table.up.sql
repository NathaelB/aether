-- Add up migration script here
CREATE TABLE IF NOT EXISTS deployments (
    id UUID PRIMARY KEY,
    organisation_id UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    kind VARCHAR(255) NOT NULL,
    status VARCHAR(255) NOT NULL,
    namespace VARCHAR(255) NOT NULL,
    version VARCHAR(255),

    created_by UUID NOT NULL REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deployed_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_deployments_organisation_id ON deployments(organisation_id);
CREATE INDEX idx_deployments_created_by ON deployments(created_by);

CREATE INDEX idx_deployments_status ON deployments(status);

CREATE INDEX idx_deployments_active ON deployments(status) WHERE deleted_at IS NULL;
