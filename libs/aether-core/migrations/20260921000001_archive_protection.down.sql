ALTER TABLE backups
    DROP CONSTRAINT IF EXISTS backups_envelopes_name_their_key,
    DROP CONSTRAINT IF EXISTS backups_protection_is_known;

DELETE FROM backups WHERE protection = 'store_managed';

ALTER TABLE backups
    DROP COLUMN protection;

ALTER TABLE backups
    ALTER COLUMN key_provider SET NOT NULL,
    ALTER COLUMN key_name     SET NOT NULL,
    ALTER COLUMN key_version  SET NOT NULL;
