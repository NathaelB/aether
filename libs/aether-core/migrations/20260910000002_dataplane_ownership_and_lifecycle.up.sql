-- A dedicated data plane belongs to an organisation. Until now "dedicated"
-- only meant "not shared" -- there was no way to say whose it was, and so no
-- way to place a deployment on the right one.
--
-- Nullable in the column, not in the domain: a row is read into an enum whose
-- Dedicated variant carries the id, so a dedicated data plane without an owner
-- cannot exist past the boundary. The constraint below stops one being written
-- in the first place.
ALTER TABLE data_planes ADD COLUMN organisation_id UUID;

ALTER TABLE data_planes ADD CONSTRAINT data_planes_dedicated_has_owner
    CHECK (
        (mode = 'dedicated' AND organisation_id IS NOT NULL)
        OR (mode <> 'dedicated' AND organisation_id IS NULL)
    );

-- Placement now filters on the owner as well as the region, mode, status and
-- liveness it already filtered on.
CREATE INDEX idx_data_planes_dedicated_owner
ON data_planes (organisation_id, region)
WHERE mode = 'dedicated';
