ALTER TABLE deployments
    DROP CONSTRAINT IF EXISTS deployments_window_can_close,
    DROP CONSTRAINT IF EXISTS deployments_window_day_is_known,
    DROP CONSTRAINT IF EXISTS deployments_window_is_whole,
    DROP CONSTRAINT IF EXISTS deployments_auto_upgrade_is_known;

ALTER TABLE deployments
    DROP COLUMN IF EXISTS maintenance_timezone,
    DROP COLUMN IF EXISTS maintenance_minutes,
    DROP COLUMN IF EXISTS maintenance_start,
    DROP COLUMN IF EXISTS maintenance_day,
    DROP COLUMN IF EXISTS auto_upgrade;
