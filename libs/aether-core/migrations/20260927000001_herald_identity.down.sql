DROP INDEX idx_data_planes_herald_subject;
ALTER TABLE data_planes
    DROP CONSTRAINT data_planes_herald_is_whole,
    DROP COLUMN herald_subject,
    DROP COLUMN herald_client_id;
