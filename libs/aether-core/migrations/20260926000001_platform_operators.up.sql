-- Who operates this installation, and what they were granted.
--
-- The right used to be a realm role in the token: claimed rather than granted,
-- so revoking it waited for the token to expire, and one boolean covered
-- reading the estate and reaching into a customer's deployment. Held here now,
-- and read on every request -- the same way a member's permissions already
-- are.
CREATE TABLE platform_operators (
    -- The subject the identity provider issues. Covers clients as well as
    -- people: registering a data plane is done by a service account, which is
    -- not somebody.
    subject     TEXT        PRIMARY KEY,

    -- Names rather than a bitmask. There are four of them and they are read
    -- rarely, so the space a bitmask saves buys nothing -- and the console has
    -- already been bitten once by a bit index that JavaScript could not read.
    -- This column is legible from psql, which is where somebody reads it at
    -- three in the morning.
    rights      TEXT[]      NOT NULL CHECK (cardinality(rights) > 0),

    -- NULL for the operator the installation named at startup. Nobody granted
    -- the first one; that is what bootstrapping means, and recording a lie
    -- about it would be worse than recording the gap.
    granted_by  TEXT,
    granted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
