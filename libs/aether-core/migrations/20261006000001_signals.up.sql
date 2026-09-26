-- Signals: fleet-wide observations about outcomes.
--
-- A signal is opened when a probe detects a problem (a silent data plane, an
-- unreachable deployment, a missing backup, an overdue drill, a stuck action)
-- and closed when the problem is resolved. Signals are NOT incidents: no
-- severity, no ownership, no paging.
--
-- A signal with a given dedup_key that is already open (closed_at IS NULL) is
-- updated instead of being inserted: the last_seen_at is refreshed and the
-- message is replaced. This deduplication invariant is enforced by the partial
-- unique index below.
CREATE TABLE signals (
    id                      UUID        PRIMARY KEY,

    -- What kind of problem this is. A closed set, enumerated by SignalKind.
    kind                    TEXT        NOT NULL,

    -- What the signal is about: a data plane, a deployment, or an action.
    -- Three nullable columns rather than a polymorphic (kind, UUID) pair,
    -- because exact one of the three must be set.
    subject_kind            TEXT        NOT NULL,
    subject_dataplane_id    UUID,
    subject_deployment_id   UUID,
    subject_action_id       UUID,

    -- The key used to deduplicate signals. An open signal with this key will
    -- be updated instead of having a second row inserted.
    dedup_key               TEXT        NOT NULL,

    -- A human-readable message describing the signal.
    message                 TEXT        NOT NULL,

    -- When the signal first opened.
    opened_at               TIMESTAMPTZ NOT NULL,

    -- When the signal was most recently seen. Updated when an open signal
    -- is written again with the same dedup_key.
    last_seen_at            TIMESTAMPTZ NOT NULL,

    -- When the signal was closed, if at all.
    closed_at               TIMESTAMPTZ,

    -- Exactly one subject. An entry naming none or multiple is invalid.
    CONSTRAINT signals_names_one_subject
        CHECK (
            (subject_dataplane_id IS NULL)::int +
            (subject_deployment_id IS NULL)::int +
            (subject_action_id IS NULL)::int = 2
        ),

    -- The subject kind must match the set field.
    CONSTRAINT signals_subject_kind_is_known
        CHECK (subject_kind IN ('dataplane', 'deployment', 'action')),
    CONSTRAINT signals_subject_dataplane_matches_kind
        CHECK ((subject_kind = 'dataplane') = (subject_dataplane_id IS NOT NULL)),
    CONSTRAINT signals_subject_deployment_matches_kind
        CHECK ((subject_kind = 'deployment') = (subject_deployment_id IS NOT NULL)),
    CONSTRAINT signals_subject_action_matches_kind
        CHECK ((subject_kind = 'action') = (subject_action_id IS NOT NULL))
);

-- Allow foreign keys from deployments and dataplanes to reference these signals,
-- but signals outlive what they name: a signal saying a deployment was unreachable
-- should stay even after the deployment is gone.
-- No foreign key constraints are added here; signals are immutable records.

-- Partial unique index enforcing the deduplication invariant: only one open
-- signal per dedup_key.
CREATE UNIQUE INDEX idx_signals_dedup_key_open
    ON signals (dedup_key) WHERE closed_at IS NULL;

-- Index for finding open signals and listing by time.
CREATE INDEX idx_signals_open_and_time
    ON signals (closed_at, opened_at DESC);
