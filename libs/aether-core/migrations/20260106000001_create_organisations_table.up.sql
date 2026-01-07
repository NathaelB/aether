-- Create organisations table
CREATE TABLE IF NOT EXISTS organisations (
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    slug VARCHAR(50) NOT NULL UNIQUE,
    owner_id UUID NOT NULL,
    status VARCHAR(20) NOT NULL,
    plan VARCHAR(20) NOT NULL,
    max_instances INTEGER NOT NULL,
    max_users INTEGER NOT NULL,
    max_storage_gb INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ  -- NULL means not deleted (soft delete)
);

-- Create index on owner_id for faster queries
CREATE INDEX idx_organisations_owner_id ON organisations(owner_id);

-- Create index on status for filtering
CREATE INDEX idx_organisations_status ON organisations(status);

-- Create index on created_at for sorting
CREATE INDEX idx_organisations_created_at ON organisations(created_at DESC);

-- Create index on deleted_at for soft delete queries
CREATE INDEX idx_organisations_deleted_at ON organisations(deleted_at) WHERE deleted_at IS NOT NULL;

-- Create partial index for active organisations (most common query)
CREATE INDEX idx_organisations_active ON organisations(status) WHERE status = 'active';
