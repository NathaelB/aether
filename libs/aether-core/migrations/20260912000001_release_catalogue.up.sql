-- What the platform publishes, per product.
--
-- The primary key is the product and the version rather than a surrogate id.
-- A version of a product is the same release wherever it is referred to, so
-- two rows for one of them is not a duplicate to reconcile later, it is a
-- contradiction: two answers to "may this be installed". The key makes it
-- unrepresentable rather than something a caller has to remember to check.
CREATE TABLE releases (
    kind        TEXT        NOT NULL,
    version     TEXT        NOT NULL,
    status      TEXT        NOT NULL,
    risk        TEXT        NOT NULL,
    -- TEXT rather than VARCHAR(n): release notes are prose written by a human
    -- and the one thing worse than long notes is notes cut off mid-sentence.
    notes       TEXT        NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (kind, version),

    CONSTRAINT releases_version_is_semver
        CHECK (version ~ '^[0-9]+\.[0-9]+\.[0-9]+([-+].*)?$'),
    CONSTRAINT releases_status_is_known
        CHECK (status IN ('upcoming', 'available', 'deprecated', 'withdrawn')),
    CONSTRAINT releases_risk_is_known
        CHECK (risk IN ('none', 'config', 'breaking'))
);

-- Listing a product's releases newest first is what both the operator screen
-- and every eligibility question do.
CREATE INDEX idx_releases_kind_status ON releases (kind, status);
