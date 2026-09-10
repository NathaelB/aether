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
ingress/cloud-iam-ferriskey             traefik     ferriskey.aether.local
```

k3s installs Traefik, so the `Ingress` the operator creates is actually served.
Reaching it needs `ferriskey.aether.local` pointed at the mapped host port:

```
127.0.0.1 ferriskey.aether.local   # in /etc/hosts, then https://ferriskey.aether.local:8444
```

The certificate will not validate: the example asks for the `letsencrypt-prod`
cluster issuer, which cannot answer an ACME challenge for a name that only
resolves on your machine.

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

## What is not covered yet

This gets the **operator** and the identity stack running against a real
cluster. Installing Herald and Genesis alongside them is `charts/aether-dataplane`;
until a deployment created through the API has been driven end to end, a local
data plane is still exercised by applying `IdentityInstance` resources by hand.
