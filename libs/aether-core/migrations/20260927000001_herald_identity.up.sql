-- Which identity may speak for a data plane.
--
-- Every cluster used to authenticate as the same client with the same secret,
-- and name the data plane it was acting for in the URL. The control plane knew
-- it was talking to a Herald and took its word for which one -- so one
-- compromised cluster could claim another's work, acknowledge it unfinished,
-- keep a dead cluster eligible for placement, and report a live deployment
-- deleted.
ALTER TABLE data_planes
    -- What a human reads in the realm, and what the chart is configured with.
    ADD COLUMN herald_client_id TEXT,

    -- What a token actually carries, and the only one an authorisation
    -- decision may use. Unique because two data planes answering to one
    -- identity is the thing this column exists to make impossible.
    ADD COLUMN herald_subject   TEXT UNIQUE,

    -- Both or neither. A subject with no client id is an identity nobody can
    -- find in the realm; a client id with no subject is one nothing can
    -- authorise against.
    ADD CONSTRAINT data_planes_herald_is_whole
        CHECK ((herald_client_id IS NULL) = (herald_subject IS NULL));

-- Every request from a Herald resolves its data plane through this.
CREATE INDEX idx_data_planes_herald_subject ON data_planes(herald_subject);
