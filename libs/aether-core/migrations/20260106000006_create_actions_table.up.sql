-- Add up migration script here
CREATE TABLE IF NOT EXISTS actions (
    id UUID PRIMARY KEY,
    deployment_id UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    dataplane_id UUID NOT NULL REFERENCES data_planes(id) ON DELETE CASCADE,
    action_type VARCHAR(255) NOT NULL,
    target_kind VARCHAR(255) NOT NULL,
    target_id UUID NOT NULL,
    payload JSONB NOT NULL,
    version INTEGER NOT NULL,

    status VARCHAR(255) NOT NULL,
    status_at TIMESTAMPTZ,
    status_agent_id VARCHAR(255),
    status_reason TEXT,

    source_type VARCHAR(255) NOT NULL,
    source_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    source_client_id VARCHAR(255),

    constraints_not_after TIMESTAMPTZ,
    constraints_priority SMALLINT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    leased_until TIMESTAMPTZ
);

CREATE INDEX idx_actions_deployment_id ON actions(deployment_id);
CREATE INDEX idx_actions_status ON actions(status);
CREATE INDEX idx_actions_created_at ON actions(created_at);
