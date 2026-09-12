-- Who may reach a deployment.
--
-- NULL is open, which is what every deployment is until someone says
-- otherwise, and is why the column has no default beyond it.
--
-- CIDR[] rather than TEXT[]: Postgres refuses what is not a network and
-- refuses host bits set to the right of the mask, which are the same two
-- rules the domain type enforces. Stated in both places on purpose -- the
-- domain is where a person gets a usable message, the column is what holds
-- when something writes around it.
ALTER TABLE deployments
    ADD COLUMN allowed_cidrs CIDR[] NULL;

-- The invariant the domain type exists for, written where a migration or a
-- stray UPDATE would otherwise put a deployment nobody can reach: an empty
-- array is not "restricted to nothing", it is a row that should have been
-- NULL.
-- cardinality, not array_length: array_length('{}', 1) is NULL rather than 0,
-- and a CHECK evaluating to NULL passes. Written the obvious way, this
-- constraint accepted the exact row it exists to refuse.
ALTER TABLE deployments
    ADD CONSTRAINT deployments_allow_list_is_never_empty
        CHECK (allowed_cidrs IS NULL OR cardinality(allowed_cidrs) >= 1);
