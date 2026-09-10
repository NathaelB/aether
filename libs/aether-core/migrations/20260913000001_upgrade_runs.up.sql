-- One row per upgrade attempt, kept whether it succeeded or not.
--
-- A deployment holds one status and one version: the moment a second upgrade
-- starts, nothing about the first is visible there any more. This table's
-- identity is the run itself, not the deployment, so a failed attempt is
-- never overwritten by the one that replaces it.
CREATE TABLE upgrade_runs (
    id              UUID        PRIMARY KEY,
    deployment_id   UUID        NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,

    from_version    TEXT        NOT NULL,
    to_version      TEXT        NOT NULL,
    change          TEXT        NOT NULL,

    trigger_kind    TEXT        NOT NULL,
    -- Who asked, when the trigger is manual. Left NULL for a scheduled run.
    -- Deliberately not tied to trigger_kind by a CHECK: this cascades to NULL
    -- when the user is removed (same as actions.source_user_id), and a
    -- constraint linking it to trigger_kind would turn that removal into a
    -- failed deletion instead of the run simply forgetting who it was.
    triggered_by    UUID        REFERENCES users(id) ON DELETE SET NULL,

    started_at      TIMESTAMPTZ NOT NULL,
    -- Both NULL while the run is in progress, both set the moment it ends.
    outcome         TEXT,
    detail          TEXT,
    ended_at        TIMESTAMPTZ,

    CONSTRAINT upgrade_runs_from_is_semver
        CHECK (from_version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'),
    CONSTRAINT upgrade_runs_to_is_semver
        CHECK (to_version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'),
    CONSTRAINT upgrade_runs_change_is_known
        CHECK (change IN ('patch', 'minor', 'major')),
    CONSTRAINT upgrade_runs_trigger_is_known
        CHECK (trigger_kind IN ('manual', 'scheduled')),
    CONSTRAINT upgrade_runs_outcome_is_known
        CHECK (outcome IS NULL OR outcome IN ('succeeded', 'failed', 'rolled_back')),
    CONSTRAINT upgrade_runs_ends_with_an_outcome
        CHECK ((outcome IS NULL) = (ended_at IS NULL))
);

-- A deployment's history, newest first. started_at is a real instant, so
-- unlike a version it orders correctly in SQL without help from the
-- application.
CREATE INDEX idx_upgrade_runs_deployment_started
    ON upgrade_runs (deployment_id, started_at DESC);
