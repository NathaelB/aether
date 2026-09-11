-- Validates every element of a text array against the same semver pattern
-- `releases.version` already enforces.
--
-- A function rather than a subquery written directly in the CHECK: Postgres
-- refuses a subquery there, and `steps_through` needs exactly this check
-- element-wise.
CREATE FUNCTION release_rollout_all_semver(candidates TEXT[])
RETURNS boolean
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT COALESCE(bool_and(v ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'), true)
    FROM unnest(candidates) AS v
$$;

-- Progressive rollout: how much of the estate a release is offered to, on top
-- of being available. Columns rather than a side table -- a release has at
-- most one rollout, so there is nothing to join.
ALTER TABLE releases
    ADD COLUMN rollout_percentage SMALLINT NOT NULL DEFAULT 100,
    -- NULL means every plan is targeted. An array rather than a side table:
    -- the set is small, always read whole, and never filtered on in SQL.
    ADD COLUMN rollout_plans TEXT[] NULL,
    ADD COLUMN rollout_pilot_organisations UUID[] NOT NULL DEFAULT '{}',
    -- NULL means any operator version may install this release.
    ADD COLUMN minimum_operator_version TEXT NULL,
    -- Versions to pass through on the way to this one, in the order the
    -- product's authors declared them. Empty means it is reachable directly.
    ADD COLUMN steps_through TEXT[] NOT NULL DEFAULT '{}';

ALTER TABLE releases
    ADD CONSTRAINT releases_rollout_percentage_is_a_percentage
        CHECK (rollout_percentage BETWEEN 0 AND 100),
    ADD CONSTRAINT releases_rollout_plans_are_known
        CHECK (
            rollout_plans IS NULL
            OR rollout_plans <@ ARRAY['free', 'starter', 'business', 'enterprise']::TEXT[]
        ),
    ADD CONSTRAINT releases_minimum_operator_version_is_semver
        CHECK (
            minimum_operator_version IS NULL
            OR minimum_operator_version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'
        ),
    ADD CONSTRAINT releases_steps_through_are_semver
        CHECK (release_rollout_all_semver(steps_through));

-- What operator/chart version a data plane last reported running, learned
-- from its own heartbeat. NULL until the first heartbeat that carries one --
-- there is one operator per cluster, shared by every tenant on it, which is
-- why this lives on the data plane rather than on any one deployment.
ALTER TABLE data_planes
    ADD COLUMN operator_version TEXT NULL;

ALTER TABLE data_planes
    ADD CONSTRAINT data_planes_operator_version_is_semver
        CHECK (
            operator_version IS NULL
            OR operator_version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'
        );
