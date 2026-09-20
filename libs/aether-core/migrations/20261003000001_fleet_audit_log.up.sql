-- What an operator did to the installation itself.
--
-- Deliberately not `audit_log` with a nullable `organisation_id`. Every query
-- against that table filters by organisation, so a nullable scope on it would
-- be one forgotten `WHERE` away from putting a fleet action in a customer's
-- trail, or a customer's trail in front of somebody reading fleet actions. Two
-- tables cannot make that mistake: nothing here has an organisation to filter
-- on, and nothing there can be reached without one.
--
-- Append only, for the same reason `audit_log` is: a trail the party it might
-- implicate can rewrite is not a trail. Re-issuing a credential and disabling
-- a cluster are the two acts that take a customer's deployment offline without
-- touching the deployment, and this is the only account of who did either.
CREATE TABLE fleet_audit_log (
    id                  UUID        PRIMARY KEY,

    -- Named by the subject the identity provider issued, not resolved to a
    -- `users` row. Platform rights are keyed on subject in
    -- `platform_operators`, so this is the name the platform actually decides
    -- with -- and it is one the trail can always write, where a lookup that
    -- comes back empty would fail the operation it was recording.
    actor_type          TEXT        NOT NULL,
    actor_subject       TEXT,
    actor_client_id     TEXT,

    -- A closed set, enumerated by `FleetAuditAction`. Unlike `audit_log.action`
    -- -- which every bounded context writes into and so has to stay open text
    -- -- the fleet's actions belong to the installation and are named in one
    -- place.
    action              TEXT        NOT NULL,

    -- A cluster or a subject. Two nullable columns rather than the polymorphic
    -- `(kind, UUID)` pair `audit_log` uses, because an operator is named by a
    -- subject and a subject is not a UUID: stuffing one into that column would
    -- mean inventing an id for a row the identity provider owns.
    --
    -- No foreign key on either, like `audit_log.target_id`. The trail has to
    -- outlive what it names -- the entry recording that a cluster was retired
    -- is the one worth keeping once the cluster is gone -- and an
    -- `ON DELETE SET NULL` here would collide with the check below, turning
    -- the removal it fired on into a failed deletion.
    target_dataplane_id UUID,
    target_subject      TEXT,

    -- Both set or both NULL, matching audit_log: "something changed" without
    -- saying what it changed from is not worth storing.
    change_before       JSONB,
    change_after        JSONB,

    recorded_at         TIMESTAMPTZ NOT NULL,

    CONSTRAINT fleet_audit_log_actor_is_known
        CHECK (actor_type IN ('operator', 'api', 'system')),
    CONSTRAINT fleet_audit_log_actor_subject_matches_type
        CHECK ((actor_type = 'operator') = (actor_subject IS NOT NULL)),
    CONSTRAINT fleet_audit_log_actor_client_matches_type
        CHECK ((actor_type = 'api') = (actor_client_id IS NOT NULL)),

    -- Exactly one target. An entry naming neither says nothing, and one naming
    -- both is two entries written as one.
    CONSTRAINT fleet_audit_log_names_one_target
        CHECK ((target_dataplane_id IS NULL) != (target_subject IS NULL)),

    CONSTRAINT fleet_audit_log_change_is_paired
        CHECK ((change_before IS NULL) = (change_after IS NULL))
);

-- The whole trail, newest first, matching the keyset pagination in
-- PostgresFleetAuditRepository::list. There is no per-tenant variant to index:
-- the installation is the only scope this table has.
CREATE INDEX idx_fleet_audit_log_recorded
    ON fleet_audit_log (recorded_at DESC, id DESC);
