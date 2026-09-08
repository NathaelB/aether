#!/usr/bin/env bash
# Creates a k3d cluster dedicated to Aether and installs what the operator
# needs to reconcile an IdentityInstance.
#
# It never touches the current kubectl context: every command targets the
# cluster's own context explicitly. A laptop usually has other clusters on it,
# and installing an operator into the wrong one is not something you notice
# until later.
set -euo pipefail

CLUSTER_NAME="${AETHER_CLUSTER_NAME:-aether-local}"
CONTEXT="k3d-${CLUSTER_NAME}"
CNPG_VERSION="${CNPG_VERSION:-1.25.1}"
# Not 8080: a developer machine usually already has something on it, and k3d
# fails the whole cluster creation on a port collision rather than picking
# another one.
HTTP_PORT="${AETHER_HTTP_PORT:-8081}"
HTTPS_PORT="${AETHER_HTTPS_PORT:-8444}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "❌ $1 is required but not installed" >&2
        exit 1
    }
}

usage() {
    cat <<USAGE
usage: $(basename "$0") <up|down|status>

  up      create the cluster and install the CRDs and CloudNativePG
  down    delete the cluster
  status  show what is installed

Environment:
  AETHER_CLUSTER_NAME   cluster name (default: aether-local)
  AETHER_HTTP_PORT      host port mapped to the ingress (default: ${HTTP_PORT})
  AETHER_HTTPS_PORT     host port mapped to TLS (default: ${HTTPS_PORT})
  CNPG_VERSION          CloudNativePG version (default: ${CNPG_VERSION})
USAGE
}

up() {
    require k3d
    require kubectl

    for port in "${HTTP_PORT}" "${HTTPS_PORT}"; do
        if lsof -nP -iTCP:"${port}" -sTCP:LISTEN >/dev/null 2>&1; then
            echo "❌ port ${port} is already in use." >&2
            echo "   Override it: AETHER_HTTP_PORT=... AETHER_HTTPS_PORT=... make local-up" >&2
            exit 1
        fi
    done

    if k3d cluster list "${CLUSTER_NAME}" >/dev/null 2>&1; then
        echo "✅ cluster ${CLUSTER_NAME} already exists"
    else
        echo "📦 creating k3d cluster ${CLUSTER_NAME}"
        # Traefik is kept: the operator creates an Ingress, and without an
        # ingress controller that resource is accepted and then serves nothing,
        # which looks like a working deployment right up until you curl it.
        # --kubeconfig-switch-context=false: k3d switches the active context on
        # create by default, which would silently repoint every kubectl in the
        # shell at a cluster the user did not ask to be in.
        k3d cluster create "${CLUSTER_NAME}" \
            --agents 1 \
            --port "${HTTP_PORT}:80@loadbalancer" \
            --port "${HTTPS_PORT}:443@loadbalancer" \
            --kubeconfig-switch-context=false \
            --wait
    fi

    echo "🔌 installing CloudNativePG ${CNPG_VERSION}"
    # The operator provisions each deployment's database as a
    # postgresql.cnpg.io/v1 Cluster, so without this every IdentityInstance
    # stalls in DatabaseProvisioning with no obvious cause.
    kubectl --context "${CONTEXT}" apply --server-side -f \
        "https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/release-${CNPG_VERSION%.*}/releases/cnpg-${CNPG_VERSION}.yaml"

    echo "⏳ waiting for CloudNativePG to be ready"
    kubectl --context "${CONTEXT}" -n cnpg-system wait --for=condition=Available \
        deployment/cnpg-controller-manager --timeout=180s

    # The examples deploy into this namespace; creating it here keeps the
    # first run from failing on something unrelated to Aether.
    echo "📁 creating the test-aether namespace"
    kubectl --context "${CONTEXT}" create namespace test-aether \
        --dry-run=client -o yaml | kubectl --context "${CONTEXT}" apply -f - >/dev/null

    echo "📄 installing Aether CRDs"
    "${REPO_ROOT}/scripts/generate-crds.sh" >/dev/null
    kubectl --context "${CONTEXT}" apply -f "${REPO_ROOT}/k8s/crds/"

    echo
    echo "✅ ${CLUSTER_NAME} is ready"
    echo
    echo "   The context is NOT switched. Point commands at it explicitly:"
    echo "     kubectl --context ${CONTEXT} get identityinstances -A"
    echo
    echo "   Run the operator against it:"
    echo "     KUBECONFIG=\$(k3d kubeconfig write ${CLUSTER_NAME}) cargo run -p aether-operator"
    echo
    echo "   Then apply an example:"
    echo "     kubectl --context ${CONTEXT} apply -f k8s/examples/identity-instance-ferriskey.yaml"
}

down() {
    require k3d
    echo "🗑️  deleting cluster ${CLUSTER_NAME}"
    k3d cluster delete "${CLUSTER_NAME}"
}

status() {
    require kubectl
    if ! kubectl config get-contexts "${CONTEXT}" >/dev/null 2>&1; then
        echo "❌ cluster ${CLUSTER_NAME} does not exist — run: make local-up"
        exit 1
    fi

    echo "context: ${CONTEXT}"
    echo
    echo "CloudNativePG:"
    kubectl --context "${CONTEXT}" -n cnpg-system get deployment cnpg-controller-manager \
        --no-headers 2>/dev/null || echo "  not installed"
    echo
    echo "Aether CRDs:"
    kubectl --context "${CONTEXT}" get crd -o name 2>/dev/null | grep aether || echo "  none"
    echo
    echo "IngressClass:"
    kubectl --context "${CONTEXT}" get ingressclass --no-headers 2>/dev/null || echo "  none"
}

case "${1:-}" in
    up) up ;;
    down) down ;;
    status) status ;;
    *) usage; exit 1 ;;
esac
