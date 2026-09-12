-- Archives that exist, and the policy that decides how long they keep
-- existing.
--
-- There is no status column, and that is the point. A row here is an archive
-- that finished; an attempt that did not is an `actions` row and an audit
-- entry, which is where the rest of the platform already records work that was
-- tried. A `status = 'failed'` row would be a restore somebody eventually
-- attempts, and every query reading this table would grow a filter it can
-- forget. Absence cannot be forgotten.
CREATE TABLE backups (
    id              UUID        PRIMARY KEY,
    deployment_id   UUID        NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    organisation_id UUID        NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,

    -- What was running when it was taken. A restore never goes backwards from
    -- here, and the product has to match: a Keycloak dump is not a Ferriskey
    -- one.
    kind            TEXT        NOT NULL,
    version         TEXT        NOT NULL,

    -- What a base backup is locked to. A logical dump ignores it, which is why
    -- the constraint lives in the domain against the method rather than in a
    -- CHECK here.
    postgres_major  INTEGER     NOT NULL,

    method          TEXT        NOT NULL,

    -- Which key wrapped the data key, recorded rather than resolved later.
    -- "The current version" is not an answer to "what encrypted this", and an
    -- installation that rotated twice could not reconstruct it.
    key_provider    TEXT        NOT NULL,
    key_name        TEXT        NOT NULL,
    key_version     INTEGER     NOT NULL,

    -- Relative to the archive prefix, which is rebuilt from organisation_id
    -- and deployment_id rather than stored. Storing the whole path would be a
    -- second way to address an archive, and the weaker of the two: a row whose
    -- path disagreed with its own columns would point somewhere nobody meant.
    object_key      TEXT        NOT NULL,

    size_bytes      BIGINT      NOT NULL,

    started_at      TIMESTAMPTZ NOT NULL,
    -- Not nullable. An archive still being written is not a backup, and this
    -- column is where that sentence is enforced against anything writing
    -- around the domain.
    finished_at     TIMESTAMPTZ NOT NULL,

    CONSTRAINT backups_version_is_semver
        CHECK (version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'),
    CONSTRAINT backups_method_is_known
        CHECK (method IN ('logical', 'physical')),
    -- The same rule the domain states as NonZeroU64. An archive of zero bytes
    -- is not a small archive, it is a backup that did not happen.
    CONSTRAINT backups_are_not_empty
        CHECK (size_bytes > 0),
    CONSTRAINT backups_finish_after_they_start
        CHECK (finished_at >= started_at),
    -- Two rows for one object is two answers to "what is in this archive".
    CONSTRAINT backups_address_one_object
        UNIQUE (deployment_id, object_key)
);

-- Every read of this table is one deployment's archives, newest first:
-- listing them, applying retention, and finding the one to restore from.
CREATE INDEX idx_backups_deployment_finished ON backups (deployment_id, finished_at DESC);
CREATE INDEX idx_backups_organisation ON backups (organisation_id);

-- When archives are taken and how much history is kept.
--
-- One row per deployment, so the primary key is the deployment. A deployment
-- with two schedules is two answers to "when is this backed up", and the key
-- makes it unrepresentable rather than something a caller has to check.
CREATE TABLE backup_schedules (
    deployment_id   UUID        PRIMARY KEY REFERENCES deployments(id) ON DELETE CASCADE,
    organisation_id UUID        NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,

    -- The shape of a cadence depends on which one it is, and a weekly one
    -- carries a day a daily one has no column for. JSONB rather than three
    -- nullable columns whose valid combinations live in a comment.
    cadence         JSONB       NOT NULL,

    -- A real zone, not an offset. 02:30 local means 02:30 after a daylight
    -- saving change too, and an offset cannot say that. Same reasoning as the
    -- maintenance window.
    zone            TEXT        NOT NULL DEFAULT 'UTC',

    keep_last       INTEGER     NOT NULL,
    keep_for_days   BIGINT      NOT NULL,

    method          TEXT        NOT NULL,

    -- A schedule that is off keeps its settings, so turning backups back on
    -- does not mean filling the form in again from memory.
    enabled         BOOLEAN     NOT NULL DEFAULT TRUE,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT backup_schedules_method_is_known
        CHECK (method IN ('logical', 'physical')),
    -- NonZeroU32 in the domain, and the reason is the same on both sides:
    -- there must be no way to express a policy that deletes the last archive a
    -- deployment has.
    CONSTRAINT backup_schedules_keep_at_least_one
        CHECK (keep_last > 0),
    CONSTRAINT backup_schedules_window_runs_forwards
        CHECK (keep_for_days >= 0)
);

-- The scheduler asks for every schedule that is on, and nothing else.
CREATE INDEX idx_backup_schedules_enabled ON backup_schedules (enabled) WHERE enabled;
