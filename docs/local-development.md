# Running Aether on a local Kubernetes cluster

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

## What is not covered yet

This gets the **operator** running against a real cluster. The rest of the
chain — control plane, Herald, Genesis — is not deployed here: that is the Helm
chart in #42, and until it exists a local data plane is driven by applying
`IdentityInstance` resources by hand rather than by creating a deployment
through the API.
