DELETE FROM actions WHERE deployment_id IS NULL;

ALTER TABLE actions DROP CONSTRAINT IF EXISTS actions_deployment_or_dataplane_target;

ALTER TABLE actions ALTER COLUMN deployment_id SET NOT NULL;
