-- Deployment reachability check history.
--
-- A raw append-only record of each deployment health check: did it respond?
-- One row per check per deployment per probe tick. No dedup, no updates.
-- Used to compute rolling-window uptime percentages.
--
CREATE TABLE deployment_reachability_checks (
    id                      BIGSERIAL   PRIMARY KEY,
    deployment_id           UUID        NOT NULL,
    checked_at              TIMESTAMPTZ NOT NULL,
    reachable               BOOLEAN     NOT NULL,

    FOREIGN KEY (deployment_id) REFERENCES deployments (id) ON DELETE CASCADE
);

-- Index for finding checks by deployment and time range.
-- Used to compute uptime over windows (last 24h, 7d, 30d).
CREATE INDEX idx_deployment_reachability_checks_by_id_and_time
    ON deployment_reachability_checks (deployment_id, checked_at DESC);

-- Index for purging old records.
CREATE INDEX idx_deployment_reachability_checks_by_time
    ON deployment_reachability_checks (checked_at);
