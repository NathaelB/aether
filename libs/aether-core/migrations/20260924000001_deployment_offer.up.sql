-- What a customer chose, and which environment they chose it for.
--
-- The environment used to be recoverable only by splitting the namespace on
-- its first hyphen, which cannot tell an environment nobody named from a
-- deployment called `api-gateway`. It is stored now.
ALTER TABLE deployments
    ADD COLUMN environment TEXT NOT NULL DEFAULT 'development',
    -- Nullable, and it stays nullable. A deployment created before offers
    -- existed has resources nobody picked from a list, and inventing an offer
    -- to describe them would be a claim about what that customer bought.
    ADD COLUMN offer TEXT;

-- Recovered from the namespace for the rows that already have one, since that
-- is where the environment lived. Anything whose namespace does not begin with
-- one of the three keeps the default rather than being guessed at.
UPDATE deployments
SET environment = split_part(namespace, '-', 1)
WHERE split_part(namespace, '-', 1) IN ('production', 'staging', 'development');

ALTER TABLE deployments
    ADD CONSTRAINT deployments_environment_known
    CHECK (environment IN ('production', 'staging', 'development'));

-- Not a foreign key and not an enum: the set of offers is a domain concept
-- that changes with the catalogue, and a database that refuses an offer the
-- code knows about would fail a deployment at the last possible moment.
