CREATE TABLE data_planes (
    id UUID PRIMARY KEY,
    mode TEXT NOT NULL,
    region TEXT NOT NULL,
    status TEXT NOT NULL,
    capacity INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_data_planes_shared_active
ON data_planes (region)
WHERE mode = 'shared' AND status = 'active';
