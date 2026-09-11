ALTER TABLE data_planes DROP CONSTRAINT IF EXISTS data_planes_operator_version_is_semver;
ALTER TABLE data_planes DROP COLUMN IF EXISTS operator_version;

ALTER TABLE releases DROP CONSTRAINT IF EXISTS releases_steps_through_are_semver;
ALTER TABLE releases DROP CONSTRAINT IF EXISTS releases_minimum_operator_version_is_semver;
ALTER TABLE releases DROP CONSTRAINT IF EXISTS releases_rollout_plans_are_known;
ALTER TABLE releases DROP CONSTRAINT IF EXISTS releases_rollout_percentage_is_a_percentage;

ALTER TABLE releases
    DROP COLUMN IF EXISTS steps_through,
    DROP COLUMN IF EXISTS minimum_operator_version,
    DROP COLUMN IF EXISTS rollout_pilot_organisations,
    DROP COLUMN IF EXISTS rollout_plans,
    DROP COLUMN IF EXISTS rollout_percentage;

DROP FUNCTION IF EXISTS release_rollout_all_semver(TEXT[]);
