-- Drop indices
DROP INDEX IF EXISTS idx_organisations_active;
DROP INDEX IF EXISTS idx_organisations_deleted_at;
DROP INDEX IF EXISTS idx_organisations_created_at;
DROP INDEX IF EXISTS idx_organisations_status;
DROP INDEX IF EXISTS idx_organisations_owner_id;

-- Drop organisations table
DROP TABLE IF EXISTS organisations;
