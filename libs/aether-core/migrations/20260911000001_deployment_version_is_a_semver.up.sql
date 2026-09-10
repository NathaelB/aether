-- Every deployment version must be a semver from here on.
--
-- Two rows hold 'latest'. What they actually run cannot be recovered: the
-- version is used directly as the container image tag, so 'latest' resolved to
-- whatever that tag pointed at when the pod started, and nothing recorded it.
-- Writing a plausible number here would be inventing a fact, and an invented
-- current version produces a wrong upgrade path, which is the one place it
-- would do real damage.
--
-- 0.0.0 is the honest answer: parsable, ordered below every real release, and
-- obviously not a version anybody shipped. These deployments predate versions
-- being real and should be recreated.
UPDATE deployments
SET version = '0.0.0',
    updated_at = now()
WHERE version IS NULL
   OR version !~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$';

-- NOT NULL as well as the check: a CHECK constraint passes on NULL, so the
-- column would otherwise still accept a row with no version at all.
ALTER TABLE deployments
    ALTER COLUMN version SET NOT NULL;

ALTER TABLE deployments
    ADD CONSTRAINT deployments_version_is_semver
    CHECK (version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$');
