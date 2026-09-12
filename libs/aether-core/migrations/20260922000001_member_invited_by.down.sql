DROP INDEX IF EXISTS idx_members_invited_by;
ALTER TABLE members DROP COLUMN IF EXISTS invited_by;
