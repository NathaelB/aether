-- What a customer delegates about upgrading a deployment, and when.
--
-- Defaulted to 'manual' rather than to anything automatic: a deployment that
-- predates this column never agreed to be upgraded on its own, and inheriting
-- an automation nobody chose is the one migration mistake that cannot be
-- undone by editing a row afterwards.
ALTER TABLE deployments
    ADD COLUMN auto_upgrade TEXT NOT NULL DEFAULT 'manual',
    -- All four or none. A window missing its zone is not a window, and letting
    -- the columns disagree pushes the question to every read.
    ADD COLUMN maintenance_day TEXT,
    ADD COLUMN maintenance_start TIME,
    ADD COLUMN maintenance_minutes INTEGER,
    ADD COLUMN maintenance_timezone TEXT;

ALTER TABLE deployments
    ADD CONSTRAINT deployments_auto_upgrade_is_known
    CHECK (auto_upgrade IN ('manual', 'patch', 'patch_and_minor'));

ALTER TABLE deployments
    ADD CONSTRAINT deployments_window_is_whole
    CHECK (
        num_nonnulls(maintenance_day, maintenance_start, maintenance_minutes, maintenance_timezone)
        IN (0, 4)
    );

ALTER TABLE deployments
    ADD CONSTRAINT deployments_window_day_is_known
    CHECK (maintenance_day IS NULL OR maintenance_day IN ('mon','tue','wed','thu','fri','sat','sun'));

-- A window that cannot close is not a window. The domain refuses it too; this
-- is what stops a migration or a hand-written UPDATE from writing one anyway.
ALTER TABLE deployments
    ADD CONSTRAINT deployments_window_can_close
    CHECK (maintenance_minutes IS NULL OR (maintenance_minutes > 0 AND maintenance_minutes < 10080));
