ALTER TABLE actions ALTER COLUMN deployment_id DROP NOT NULL;

ALTER TABLE actions
    ADD CONSTRAINT actions_deployment_or_dataplane_target
    CHECK (deployment_id IS NOT NULL OR target_kind = 'dataplane');
