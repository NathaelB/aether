-- The route an upgrade takes, not just where it ends.
--
-- Some versions cannot be reached in one hop: the catalogue says which ones
-- have to be passed through, and the run records the answer it was given when
-- the upgrade was accepted. Recomputing it later would let a release
-- withdrawn halfway through reroute an upgrade already under way.
--
-- text[] rather than jsonb: this is an ordered list of versions and nothing
-- else, and jsonb would invite a shape to grow here that belongs in the
-- domain type.
ALTER TABLE upgrade_runs
    ADD COLUMN steps text[] NOT NULL DEFAULT '{}';

-- Runs recorded before this column existed were single hops by construction,
-- so the target is the whole path.
UPDATE upgrade_runs SET steps = ARRAY[to_version] WHERE cardinality(steps) = 0;
