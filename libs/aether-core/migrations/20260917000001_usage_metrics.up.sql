-- One point per deployment per minute, pre-aggregated by the data plane
-- before it ever reaches here -- the storage decision for #91 (V7): roughly
-- 500k rows a month for a hundred instances, which Postgres carries without
-- a second data system. Ports exist so that changes if it stops being true;
-- this table is deliberately the only thing that would move.
--
-- No surrogate key: identity is (deployment_id, metric, bucket_start), and a
-- second write for the same three is Herald replaying its report window
-- after a restart, not a new fact -- the insert this table is built for does
-- ON CONFLICT on exactly that key and overwrites the value.
CREATE TABLE usage_metrics (
    deployment_id   UUID        NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,

    -- A closed set, matching aether_domain::metrics::MetricKind. A value
    -- outside it is a bug in the reporter, not a new kind of usage, which is
    -- what the constraint below is for.
    metric          TEXT        NOT NULL,

    -- Floored to the minute in the domain before it ever reaches this
    -- statement; the constraint below is the backstop that stops a stray
    -- sub-minute value from opening a second row for what is already one
    -- bucket.
    bucket_start    TIMESTAMPTZ NOT NULL,

    value           BIGINT      NOT NULL,

    PRIMARY KEY (deployment_id, metric, bucket_start),

    CONSTRAINT usage_metrics_metric_is_known
        CHECK (metric IN ('requests', 'token_events', 'logins', 'active_users')),
    CONSTRAINT usage_metrics_value_is_not_negative
        CHECK (value >= 0),
    CONSTRAINT usage_metrics_bucket_is_a_whole_minute
        CHECK (date_trunc('minute', bucket_start) = bucket_start)
);
