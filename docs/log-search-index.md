# The search index a customer's logs land in

This is V0 of #292: an index Herald can write to. Nothing writes to it yet --
that is V1 (#294) -- and nothing queries it yet -- that is V2 (#295). What
exists after this page is infrastructure and a schema, decided now so neither
of those has to guess.

## Why Quickwit, and why not a new database

A local spike (recorded in #292) ran ingestion, a time-bucketed histogram, a
terms aggregation for facets, and full-text search against a Quickwit
instance pointed at RustFS, on the same bucket family `libs/aether-s3`
already writes backups to. All four worked. That ruled out standing up a
second stateful tier from scratch: the object store this platform already
runs everywhere is enough, and Quickwit is built to carry many small indices
well, so a fleet's worth of organisations is not the scaling problem for it
that it would be for Elasticsearch.

**Rejected: a shared index, filtered by `organisation_id` at query time.**
Isolation would then be a `WHERE` clause somebody has to remember to add to
every query, forever. A separate index per organisation makes it a boundary
Quickwit enforces structurally -- which index is even reachable -- instead.

**Rejected: a prefix inside the `aether-backups` bucket, instead of a bucket
of its own.** `S3ObjectStore::ensure_bucket` (`libs/aether-s3`) applies a
lifecycle rule to the *whole* bucket every time the control plane starts, by
replacing its lifecycle configuration outright rather than merging into it.
A log index living in that bucket would mean its own retention rule and the
backup retention rule overwriting each other on alternating restarts. A
second bucket, `aether-logs` (`aether-control-plane.fullname`-`-quickwit`'s
`quickwit.bucket` value in the chart), costs nothing extra to run -- it is
the same RustFS, the same credentials -- and keeps the two retention stories
from ever fighting over one bucket's configuration.

## The naming convention

One index per organisation: **`logs-{organisation_id}`**, where
`organisation_id` is the organisation's UUID in its canonical lowercase
hyphenated form. Quickwit index ids must start with a letter and match
`[a-zA-Z][a-zA-Z0-9-_\.]{2,254}` -- a bare UUID starts with a digit often
enough to be illegal on its own, which is what the `logs-` prefix is for.

Nothing in this repository creates one of these indices yet. Creation
happens where V1 first needs it, against the doc mapping below.

## The doc mapping (frozen)

`docker/quickwit/index-config.template.yaml` is the committed template --
substitute `{organisation_id}` with the real UUID before handing it to
`quickwit index create`, per the comment at the top of that file. The six
field names and their roles do not change without touching every version
downstream of this one; V4 (#297) adds a seventh, `fingerprint`, later.

| field | type | role |
|---|---|---|
| `timestamp` | `datetime`, fast | the index timestamp field; what the time-bucketed histogram buckets on, and what `retention` prunes by |
| `organisation_id` | `text`, `raw` tokenizer | the tenant. Filtered on exactly |
| `deployment_id` | `text`, `raw` tokenizer | which deployment the line came from. Filtered on exactly |
| `source` | `text`, `raw` tokenizer, fast | the container the line came from. Faceted on -- a terms aggregation needs a fast field |
| `level` | `text`, `raw` tokenizer, fast | the log level. Faceted on, and V2 also filters on it as a floor |
| `message` | `text`, `default` tokenizer | the line itself. The default search field, full text |

`raw` rather than a word tokenizer on the four keyword-shaped fields: an
exact filter has to match the value whole, not a fragment a tokenizer split
it into. `default` on `message` alone, because it is the one field meant to
be searched by words rather than matched by value.

The mapping mode is `lenient`: a field a future version sends that is not
one of these six is left unindexed rather than turning the whole document
into a rejected write.

## Retention

30 days, expressed in the index config's own `retention` block (`period: 30
days`, `schedule: daily`), not by an external cronjob. This is separate from
and unrelated to backup retention -- a different bucket, a different
mechanism, a different number if the two are ever changed independently.

## Running it locally

```bash
docker compose up -d quickwit
```

Quickwit answers on `localhost:7280` (`QUICKWIT_PORT` to change it). Inside
the Compose network it is `quickwit:7280`; `quickwit-bucket` (a one-shot
step, the way `aether-migrations` is one) creates the `aether-logs` bucket
against RustFS before Quickwit starts, since Quickwit's own S3 client -- unlike
`aether-s3`'s `ensure_bucket` -- never creates a bucket it does not find.

To try the acceptance criterion by hand -- this is the exact sequence this
chantier was verified with, against two organisations at once to also show
the per-index isolation is real, not just the filter:

```bash
org1=11111111-1111-1111-1111-111111111111
org2=22222222-2222-2222-2222-222222222222

for org in "$org1" "$org2"; do
  sed "s/{organisation_id}/$org/" \
    docker/quickwit/index-config.template.yaml > /tmp/logs-$org.yaml
  curl -s -X POST http://localhost:7280/api/v1/indexes \
    -H "content-type: application/yaml" --data-binary @/tmp/logs-$org.yaml
done

curl -s -X POST "http://localhost:7280/api/v1/logs-$org1/ingest?commit=force" \
  -H "content-type: application/x-ndjson" --data-binary @- <<EOF
{"timestamp":"2026-09-20T08:00:00Z","organisation_id":"$org1","deployment_id":"dep-a","source":"api","level":"info","message":"hello from org1 at 8am"}
{"timestamp":"2026-09-20T09:00:00Z","organisation_id":"$org1","deployment_id":"dep-a","source":"api","level":"warn","message":"a second line from org1 at 9am"}
{"timestamp":"2026-09-20T10:00:00Z","organisation_id":"$org1","deployment_id":"dep-a","source":"worker","level":"error","message":"a third line from org1 at 10am"}
EOF

curl -s -X POST "http://localhost:7280/api/v1/logs-$org2/ingest?commit=force" \
  -H "content-type: application/x-ndjson" --data-binary @- <<EOF
{"timestamp":"2026-09-20T08:30:00Z","organisation_id":"$org2","deployment_id":"dep-b","source":"api","level":"info","message":"hello from org2, a different organisation entirely"}
EOF

# Exact organisation_id filter: org1's index returns its 3 lines...
curl -s -X POST "http://localhost:7280/api/v1/logs-$org1/search" \
  -H "content-type: application/json" \
  -d "{\"query\":\"organisation_id:$org1\",\"max_hits\":10}"

# ...org2's index returns none of them, even asked for the same id...
curl -s -X POST "http://localhost:7280/api/v1/logs-$org2/search" \
  -H "content-type: application/json" \
  -d "{\"query\":\"organisation_id:$org1\",\"max_hits\":10}"

# A time-bucketed histogram over org1's own three lines, one per hour:
curl -s -X POST "http://localhost:7280/api/v1/logs-$org1/search" \
  -H "content-type: application/json" \
  -d '{"query":"*","max_hits":0,"aggs":{"by_hour":{"date_histogram":{"field":"timestamp","fixed_interval":"3600s"}}}}'
```

## In the chart

`charts/aether-control-plane` deploys Quickwit as `quickwit.enabled`
(default `true`): a single-replica Deployment running `quickwit run` in its
all-in-one mode (metastore, indexer, searcher and control plane in one
process -- there is no traffic yet to split it for), a ClusterIP `Service`
reachable only inside the cluster, and a Job that creates `quickwit.bucket`
the same way `quickwit-bucket` does locally. It reuses `objectStore`'s
endpoint, region and credentials -- see `values.yaml` for the full set of
knobs.
