-- Where a data plane's own Gateway answers -- a LoadBalancer address
-- Kubernetes already assigns it, Herald reads back and reports on every
-- heartbeat.
--
-- Nullable, and it stays nullable: absent means exactly today's behaviour.
-- Nothing downstream depends on it existing yet -- it is the first piece of
-- #279 (a deployment's hostname is decided, not invented), which needs to
-- know where a data plane can be reached before it can point a DNS record
-- at it.
ALTER TABLE data_planes ADD COLUMN gateway_address TEXT;
