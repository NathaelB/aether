-- Which archive a deployment came back from.
--
-- Read by the console to tell a recovery apart from a deployment somebody
-- created, and by the cutover to know what it is switching away from. NULL is
-- the ordinary case and means nobody restored it.
ALTER TABLE deployments
    ADD COLUMN restored_from UUID
        -- SET NULL rather than CASCADE: an archive can be expired by
        -- retention, or cascade away with the source deployment, long after
        -- the recovery it produced became the thing serving traffic. Deleting
        -- a live deployment because its provenance was cleaned up would be a
        -- data loss caused by bookkeeping.
        REFERENCES backups(id) ON DELETE SET NULL;
