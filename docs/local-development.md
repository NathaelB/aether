# Running Aether locally

## The short version

```bash
make demo
```

One command: the control plane in Compose, the realm in FerrisKey, a k3d
cluster, a registered data plane, and the chart installed into it. It prints
what to put in `apps/console/.env` and what to open. `make demo-down` removes
all of it.

Every step is idempotent — run it again after a failure rather than starting
from a clean machine. The rest of this document is what it does, and how to do
any one piece by hand when something goes wrong.

## The split

Aether splits across two runtimes, and running it locally mirrors that split
rather than fighting it:

| | runs where | how |
|---|---|---|
| control plane, Ferriskey, Postgres, broker | Docker Compose | `docker compose --profile ferriskey up` |
| Herald, Genesis, the operator | a k3d cluster | `make local-up` then the chart |

The data plane pulls from the control plane, so the direction is
cluster -> host: pods reach the control plane at `host.k3d.internal`, and the
control plane needs no route into the cluster at all.

Putting Genesis in Compose would not work, and not only for tidiness: it applies
`IdentityInstance` resources, so it needs a cluster to apply them to.


The operator reconciles an `IdentityInstance` into a CloudNativePG `Cluster`, a
`Deployment` and an `Ingress`. To watch that happen you need a cluster with
CloudNativePG and the Aether CRDs installed.

## One command

```bash
make local-up
```

It creates a k3d cluster named `aether-local`, installs CloudNativePG and the
Aether CRDs, and creates the `test-aether` namespace the examples use.

**It does not switch your kubectl context.** `k3d cluster create` does that by
default, which would silently repoint every `kubectl` in your shell at a cluster
you did not ask to be in — so the script disables it and prints the context to
use instead.

`make local-status` shows what is installed, `make local-down` deletes the
cluster.

### If a port is taken

The cluster maps host ports 8081 and 8444 to the ingress. If either is busy the
script says so before creating anything, rather than letting k3d roll back a
half-built cluster twenty seconds later:

```bash
AETHER_HTTP_PORT=9081 AETHER_HTTPS_PORT=9444 make local-up
```

## Running the operator against it

The operator uses the ambient kubeconfig, so point it at the cluster explicitly
rather than switching context:

```bash
KUBECONFIG=$(k3d kubeconfig write aether-local) RUST_LOG=info cargo run -p aether-operator
```

Then, in another shell:

```bash
kubectl --context k3d-aether-local apply -f k8s/examples/identity-instance-ferriskey.yaml
kubectl --context k3d-aether-local -n test-aether get identityinstance -w
```

The status walks through its phases, and `kubectl -n test-aether get all` shows
the CNPG cluster, its services and the IAM deployment appear as it goes.

## What a successful run looks like

Reconciliation walks `DatabaseProvisioning` -> `Deploying` -> `Running`, and ends
at `ready: true` with:

```
pod/cloud-iam-ferriskey-db-1            Running     the CloudNativePG database
pod/cloud-iam-ferriskey-migrate-...     Completed   schema migration
deployment/cloud-iam-ferriskey-api      1/1
deployment/cloud-iam-ferriskey-webapp   1/1
httproute/cloud-iam-ferriskey                       ferriskey.aether.local
```

## Reaching a deployment

An instance is served by an `HTTPRoute` attached to the data plane's Envoy
Gateway, and k3d publishes the node's port 80 on **8081**. So the traffic path
already works:

```bash
curl -H "Host: ferriskey.aether.local" http://localhost:8081/
```

What does not work is a browser, because nothing resolves that name. `/etc/hosts`
has no wildcards, so every deployment needs a line of its own:

```bash
make local-hosts         # show the lines, and the URLs they make work
sudo make local-hosts-apply
```

It reads the hostnames from the routes that are actually serving rather than
guessing them from a naming convention, writes them between its own markers, and
rewrites that block wholesale on each run: a deployment that is gone stops
resolving instead of pointing at nothing for ever. Lines outside the markers are
never touched, and a name you already resolve yourself is reported and left
alone. `sudo make local-hosts-remove` takes the block back out.

Then open `http://ferriskey.aether.local:8081`.

### There is no HTTPS locally

`gateway.tls.secretName` is empty by default, so the Gateway has one `http`
listener and nothing answers on 8444. A listener that cannot resolve its
certificate takes the whole Gateway out of `Programmed`, including the plain
HTTP listener beside it, which is a worse failure than not having TLS on a
laptop.

## Identity

The control plane, Herald and the console all authenticate against the local
Ferriskey. The realm and its two clients are declared in
`deploy/ferriskey/terraform` and applied with:

```bash
docker compose --profile ferriskey up -d
make bootstrap-auth
```

It prints the issuer, the console's client id and Herald's client secret. Run it
again after changing the realm and it converges — Terraform is used here rather
than a script driving the API precisely because a second run has to be safe.

The secret is printed rather than written to a file: it belongs wherever you
keep secrets, not in the working tree.

> Herald's client secret comes back from an API call in the script rather than
> from `terraform output`. At provider v0.1.0 `ferriskey_client.secret` stores
> `***` — the mask the API returns in the client body — instead of the generated
> value. The output exists and starts working the day the provider does.

Both clients are for a throwaway stack. A shared Ferriskey should follow the
provider's [bootstrap guide][bootstrap]: a `terraform-runner` service account
with scoped roles, rather than the admin account.

[bootstrap]: https://registry.terraform.io/providers/ferriskey/ferriskey/latest/docs/guides/bootstrap

### If a port is taken

Every published port in `docker-compose.yaml` can be moved:

```bash
FERRISKEY_API_PORT=4334 AETHER_POSTGRES_PORT=5434 docker compose --profile ferriskey up -d
FERRISKEY_URL=http://localhost:4334 make bootstrap-auth
```

## Object storage

Archives go to an S3 bucket. Locally that is RustFS, brought up with the rest of
the Compose side:

```bash
docker compose up -d rustfs
```

It publishes **9800** rather than 9000. Every object store in every other
project picks 9000, and a bucket answering there is very likely not this one;
inside the Compose network the container port is still 9000, so nothing else has
to know. Its console is on 9801.

RustFS speaks S3, so the adapter exercised here is the one that runs against
Scaleway later. There is no development-only path to diverge from production,
which is the reason for using it rather than a filesystem stub.

The control plane creates the bucket at startup if it is not there, and applies
the one rule every bucket this platform writes to must carry: parts of multipart
uploads that never completed are discarded after seven days. Without it, an
interrupted archive leaves parts that appear in no listing and are billed until
somebody goes looking with a tool that can see them.

An installation that cannot reach its store still starts. Backups stop,
authentication does not, and the logs say so on the first line. Setting
`OBJECT_STORE_BUCKET` to an empty string turns archiving off deliberately, which
is a different thing from a store being down and is logged differently.

### Pointing it somewhere else

Four variables, no code:

```bash
OBJECT_STORE_ENDPOINT=https://s3.fr-par.scw.cloud \
OBJECT_STORE_REGION=fr-par \
OBJECT_STORE_ACCESS_KEY=... \
OBJECT_STORE_SECRET_KEY=... \
  cargo run -p aether-control-plane
```

`OBJECT_STORE_PATH_STYLE` is true by default, which is what every self hosted
store wants. AWS itself wants it false. Getting it wrong produces a DNS failure
naming the bucket, which reads like a permissions problem and is not one.

### Encryption

Objects are written asking the store to encrypt them, and data keys are wrapped
by the OpenBao beside it:

```bash
docker compose up -d openbao      # transit engine, dev mode, port 8200
```

What that protects and what it does not is
[its own page](./backup-encryption.md), because the difference between the two
mechanisms people call encryption at rest is the part customers ask about.

### The tests that need it

```bash
make test-objectstore
make test-keys
```

They skip loudly when `OBJECT_STORE_ENDPOINT` is unset rather than passing on
nothing, the same way the Postgres integration tests do.

## What is not covered yet

This gets the **operator** and the identity stack running against a real
cluster. Installing Herald and Genesis alongside them is `charts/aether-dataplane`;
until a deployment created through the API has been driven end to end, a local
data plane is still exercised by applying `IdentityInstance` resources by hand.
