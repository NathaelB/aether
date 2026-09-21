-- The per-deployment switch V1 (#294) reads before following a deployment's
-- pods continuously. Default false: shipping to the search index is opt-in,
-- not on for every deployment in a fleet, which would be unbounded memory on
-- Herald and unbounded index volume on Quickwit.
--
-- The console does not drive this yet (V3, #296); the API is what V1 needs
-- today.
ALTER TABLE deployments ADD COLUMN log_shipping_enabled BOOLEAN NOT NULL DEFAULT false;
