-- Who changed what, when, across the whole platform -- not only the work
-- that reaches a data plane. `actions` already covers that half; this covers
-- the half that used to leave no trace at all, starting with a setting.
--
-- No update, no delete, not even for the platform itself: an audit trail
-- that the party it might implicate can rewrite is not an audit trail.
CREATE TABLE audit_log (
    id                  UUID        PRIMARY KEY,
    organisation_id     UUID        NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,

    actor_type          TEXT        NOT NULL,
    -- Left NULL once the user is removed rather than blocking the deletion,
    -- same as upgrade_runs.triggered_by: the entry keeps saying a human did
    -- this, it simply forgets which one. Deliberately not tied to actor_type
    -- by a CHECK, because ON DELETE SET NULL firing on a departed row would
    -- turn that removal into a failed deletion instead.
    actor_user_id       UUID        REFERENCES users(id) ON DELETE SET NULL,
    -- Free text, not a foreign key: a client id names a caller in the
    -- identity provider, not a row this database owns.
    actor_client_id     TEXT,

    -- Namespaced, e.g. "deployment.maintenance_window.updated".
    action              TEXT        NOT NULL,

    -- Polymorphic on purpose, like actions.target_kind/target_id: an audit
    -- trail has to name a role, a member or a billing plan as easily as a
    -- deployment, and no single foreign key can point at all of them.
    target_kind         TEXT        NOT NULL,
    target_id           UUID        NOT NULL,

    -- Both set or both NULL: "something changed" without saying what it
    -- changed from is not worth storing.
    change_before       JSONB,
    change_after        JSONB,

    recorded_at         TIMESTAMPTZ NOT NULL,

    CONSTRAINT audit_log_actor_is_known
        CHECK (actor_type IN ('user', 'system', 'api')),
    CONSTRAINT audit_log_actor_client_matches_type
        CHECK ((actor_type = 'api') = (actor_client_id IS NOT NULL)),
    CONSTRAINT audit_log_change_is_paired
        CHECK ((change_before IS NULL) = (change_after IS NULL))
);

-- An organisation's trail, newest first, matching the keyset pagination in
-- PostgresAuditRepository::list_for_organisation.
CREATE INDEX idx_audit_log_organisation_recorded
    ON audit_log (organisation_id, recorded_at DESC, id DESC);
