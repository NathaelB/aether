-- What stands between an archive and somebody who obtains the bucket.
--
-- The table was written assuming every archive is wrapped by a key manager.
-- The family of backup that actually ships is not: CloudNativePG hands the
-- archive to the object store, which encrypts it under a key the store holds.
-- Nothing wraps a key for it, so there is no key reference to record, and
-- recording the installation's configured key anyway would name a key that
-- unwraps nothing -- read back during a restore as an instruction to go and
-- fetch it.
--
-- So the key columns become nullable, and a column says which mechanism was
-- used rather than leaving it to be inferred from whether the others are there.
ALTER TABLE backups
    ADD COLUMN protection TEXT NOT NULL DEFAULT 'envelope';

ALTER TABLE backups
    ALTER COLUMN key_provider DROP NOT NULL,
    ALTER COLUMN key_name     DROP NOT NULL,
    ALTER COLUMN key_version  DROP NOT NULL;

-- The default exists only to fill the rows already there, which were all
-- written under the old assumption. Leaving it would let a new row inherit a
-- protection nobody stated.
ALTER TABLE backups
    ALTER COLUMN protection DROP DEFAULT;

ALTER TABLE backups
    ADD CONSTRAINT backups_protection_is_known
        CHECK (protection IN ('store_managed', 'envelope'));

-- The invariant the enum states in the domain, written where a migration or a
-- stray UPDATE would otherwise break it: an envelope without a key is an
-- archive nobody can open, and a key beside a store managed archive is a key
-- that unwraps nothing.
ALTER TABLE backups
    ADD CONSTRAINT backups_envelopes_name_their_key
        CHECK (
            (protection = 'envelope'
                AND key_provider IS NOT NULL
                AND key_name IS NOT NULL
                AND key_version IS NOT NULL)
            OR
            (protection = 'store_managed'
                AND key_provider IS NULL
                AND key_name IS NULL
                AND key_version IS NULL)
        );
