-- Add down migration script here
DROP INDEX IF EXISTS idx_member_roles_member_id;
DROP INDEX IF EXISTS idx_member_roles_role_id;

DROP TABLE IF EXISTS member_roles;

DROP INDEX IF EXISTS idx_roles_name_org;
DROP INDEX IF EXISTS idx_roles_global;
DROP INDEX IF EXISTS idx_roles_organisation_id;

DROP TABLE IF EXISTS roles;
