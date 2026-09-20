-- Add up migration script here
-- Supports claiming pending actions across every deployment a data plane
-- owns in one query, rather than one query per deployment.
CREATE INDEX idx_actions_dataplane_id_status ON actions(dataplane_id, status);
