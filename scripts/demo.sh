#!/usr/bin/env bash
# Brings the whole of Aether up locally and leaves you able to create a
# deployment from the console.
#
# There are two runtimes and the split is the architecture, not an accident:
# the control plane runs in Docker Compose, the data plane in a k3d cluster,
# and the data plane *pulls* its work. Nothing here ever gives the control
# plane credentials to the cluster.
#
# Every step is idempotent. Run it again after a failure rather than starting
# from a clean machine.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# Ports are overridable because a developer machine already has things on the
# usual ones. Every default here has collided with something real at least once.
export AETHER_POSTGRES_PORT="${AETHER_POSTGRES_PORT:-55435}"
export AETHER_API_PORT="${AETHER_API_PORT:-7777}"
export FERRISKEY_API_PORT="${FERRISKEY_API_PORT:-3334}"
export FERRISKEY_WEBAPP_PORT="${FERRISKEY_WEBAPP_PORT:-5556}"
# The published port, because the data plane reaches RustFS from outside the
# compose network. The control plane uses the container port on the inside; the
# two are the same store and the endpoints are deliberately not.
RUSTFS_PORT="${RUSTFS_PORT:-9800}"
# Same reasoning as RUSTFS_PORT above: Herald runs inside the k3d cluster and
# reaches this Quickwit -- started by the compose stack below, no profile of
# its own -- from outside the compose network.
QUICKWIT_PORT="${QUICKWIT_PORT:-7280}"

CONSOLE_PORT="${CONSOLE_PORT:-5173}"
CONTROL_PLANE="http://localhost:${AETHER_API_PORT}"
FERRISKEY_URL="http://localhost:${FERRISKEY_API_PORT}"
REGION="${AETHER_REGION:-local}"
CLUSTER="${AETHER_CLUSTER:-aether-local}"
RELEASE="${AETHER_RELEASE:-aether-dataplane}"
NAMESPACE="${AETHER_DATAPLANE_NAMESPACE:-aether-system}"

step() { printf '\n\033[1;35m▸ %s\033[0m\n' "$1"; }
note() { printf '  %s\n' "$1"; }
die()  { printf '\n\033[1;31m✗ %s\033[0m\n' "$1" >&2; exit 1; }

for tool in docker k3d helm kubectl terraform jq curl; do
    command -v "${tool}" >/dev/null 2>&1 || die "${tool} is required"
done

# Tearing down deletes a cluster and a database, so it is a named argument
# rather than a flag that could be reached by a typo.
if [ "${1:-up}" = "down" ]; then
    step "tearing down"
    docker compose --profile ferriskey down --volumes 2>&1 | tail -2 || true
    ./scripts/local-cluster.sh down || true
    note "the terraform state in deploy/ferriskey/terraform is left alone:"
    note "it describes a realm whose database has just been deleted, so run"
    note "  terraform -chdir=deploy/ferriskey/terraform state rm ferriskey_realm.aether"
    note "if the next run complains that the realm already exists."
    exit 0
fi

if [ "${1:-up}" != "up" ]; then
    die "usage: demo.sh [up|down]"
fi

# ---------------------------------------------------------------- control plane

# Whoever operates this installation, before anybody has been granted it.
# Platform rights are handed out by somebody who already holds one, and a fresh
# database has nobody to hand out the first. Named here rather than left to a
# realm role: the realm says who somebody is, the control plane says what they
# may do to it, and this is the seam between the two.
#
# Resolved below, from the realm, once the realm exists -- so the first Compose
# start of a fresh checkout comes up with nobody, says so in its logs, and is
# granted on the second pass a few lines later.
export AETHER_BOOTSTRAP_OPERATOR="${AETHER_BOOTSTRAP_OPERATOR:-}"

step "control plane (docker compose)"
docker compose --profile ferriskey up -d --build --wait 2>&1 | tail -3 \
    || die "compose failed to come up"

# `--wait` returns when containers are healthy, which is not the same as the
# API serving: the control plane binds its port after connecting to Postgres.
note "waiting for ${CONTROL_PLANE}"
for _ in $(seq 60); do
    status=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 2 \
        "${CONTROL_PLANE}/swagger" 2>/dev/null || echo 000)
    [ "${status}" != "000" ] && break
    sleep 2
done
[ "${status}" != "000" ] || die "the control plane never answered on ${CONTROL_PLANE}"
note "control plane is answering"

# -------------------------------------------------------------------- identity

step "identity (terraform against FerrisKey)"
# The realm only allows the redirect URIs it was told about, so the console's
# port has to be decided here rather than by whichever one Vite happens to find
# free. Getting this wrong fails at the identity provider, with an error that
# says nothing about a port.
export TF_VAR_console_redirect_uris="[\"http://localhost:${CONSOLE_PORT}\",\"http://localhost:${CONSOLE_PORT}/*\",\"http://localhost:${FERRISKEY_WEBAPP_PORT}\",\"http://localhost:${FERRISKEY_WEBAPP_PORT}/*\"]"
bootstrap=$(FERRISKEY_URL="${FERRISKEY_URL}" ./scripts/bootstrap-ferriskey.sh)
ISSUER=$(printf '%s' "${bootstrap}" | awk -F= '/^ *AUTH_ISSUER=/{print $2; exit}')
OPERATOR_SUBJECT=$(printf '%s' "${bootstrap}" | awk -F= '/^ *OPERATOR_SUBJECT=/{print $2; exit}')
OPERATOR_PEOPLE=$(printf '%s' "${bootstrap}" | awk -F= '/^ *OPERATOR_PEOPLE=/{print $2; exit}')
HERALD_SECRET_SHARED=$(printf '%s' "${bootstrap}" | awk -F= '/^ *AUTH_CLIENT_SECRET=/{print $2; exit}')
HERALD_SECRET="${HERALD_SECRET_SHARED}"
OPERATOR_SECRET=$(printf '%s' "${bootstrap}" | awk -F= '/^ *OPERATOR_CLIENT_SECRET=/{print $2; exit}')
if [ -z "${ISSUER}" ] || [ -z "${HERALD_SECRET}" ] || [ -z "${OPERATOR_SECRET}" ]; then
    die "could not read the realm bootstrap output"
fi
note "issuer ${ISSUER}"

token() {
    # Checked on status, not on curl's exit code: without -f, curl exits 0 on a
    # 401, and a script that trusts it reports success against a realm that
    # rejected it.
    local client="$1" secret="$2" response status
    response=$(curl -sS -X POST \
        "${FERRISKEY_URL}/realms/aether/protocol/openid-connect/token" \
        -d grant_type=client_credentials \
        -d "client_id=${client}" \
        -d "client_secret=${secret}" \
        -w $'\n%{http_code}')
    status="${response##*$'\n'}"
    [ "${status#2}" != "${status}" ] || die "could not obtain a token for ${client} (HTTP ${status})"
    printf '%s' "${response%$'\n'*}" | jq -r '.access_token'
}

# Obtained and thrown away: what matters is that the secret the chart is about
# to receive actually authenticates, which is cheaper to find out here than
# from a Herald that comes up and quietly claims nothing.
token herald-service "${HERALD_SECRET_SHARED}" >/dev/null
# Registering a data plane is an operator's act, not a data plane's. Herald's
# token is refused by those endpoints, and rightly: a data plane that could
# register another one could point work at a cluster nobody chose.
OPERATOR_TOKEN=$(token aether-operator-cli "${OPERATOR_SECRET}")
note "herald-service and aether-operator-cli can authenticate"

# ------------------------------------------------------------ platform rights

step "granting the platform rights"
# The control plane holds these, not the realm. A run before this existed left
# an installation whose operator screens refused everybody, which reads as a
# broken console rather than as an empty table.
if [ -n "${OPERATOR_SUBJECT}" ] && [ "${AETHER_BOOTSTRAP_OPERATOR}" != "${OPERATOR_SUBJECT}" ]; then
    export AETHER_BOOTSTRAP_OPERATOR="${OPERATOR_SUBJECT}"
    # Recreated rather than restarted: the subject is read from the
    # environment at startup, and a restart keeps the environment it had.
    docker compose --profile ferriskey up -d --force-recreate --no-deps aether >/dev/null 2>&1 \
        || die "could not restart the control plane with a bootstrap operator"
    for _ in $(seq 30); do
        status=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 2 \
            "${CONTROL_PLANE}/swagger" 2>/dev/null || echo 000)
        [ "${status}" != "000" ] && break
        sleep 2
    done
    OPERATOR_TOKEN=$(token aether-operator-cli "${OPERATOR_SECRET}")
fi
note "aether-operator-cli operates this installation"

# Anybody still carrying the old realm role gets the rights it used to imply.
# That role was the authority until the control plane took it over; seeding
# from it is what stops this run locking out the account that could already see
# these screens.
if [ -z "${OPERATOR_PEOPLE}" ]; then
    note "nobody carries the aether-operator realm role yet"
    note "grant yourself once you have logged in:"
    note "  curl -X PUT ${CONTROL_PLANE}/platform/operators/<your-subject> \\"
    note "       -H 'Authorization: Bearer <the operator cli token>' \\"
    note "       -H 'Content-Type: application/json' \\"
    note "       -d '{\"rights\":[\"view_estate\",\"operate_fleet\",\"act_on_tenant\",\"manage_operators\"]}'"
fi

for person in $(printf '%s' "${OPERATOR_PEOPLE}" | tr ',' ' '); do
    granted=$(curl -sS -o /dev/null -w '%{http_code}' -X PUT \
        "${CONTROL_PLANE}/platform/operators/${person}" \
        -H "Authorization: Bearer ${OPERATOR_TOKEN}" \
        -H 'Content-Type: application/json' \
        -d '{"rights":["view_estate","operate_fleet","act_on_tenant","manage_operators"]}')
    if [ "${granted#2}" != "${granted}" ]; then
        note "granted ${person}"
    else
        note "could not grant ${person} (HTTP ${granted})"
    fi
done

# ------------------------------------------------------------------- k3d cluster

step "data plane cluster (k3d)"
if k3d cluster list -o json | jq -e --arg n "${CLUSTER}" '.[] | select(.name == $n)' >/dev/null; then
    note "${CLUSTER} already exists"
else
    ./scripts/local-cluster.sh up
fi
KUBECONFIG_PATH=$(k3d kubeconfig write "${CLUSTER}")
export KUBECONFIG="${KUBECONFIG_PATH}"
note "kubeconfig ${KUBECONFIG_PATH}"

# ------------------------------------------------------------- data plane images

step "building the data plane images"
# Built here and imported into k3d rather than pulled from ghcr, for one reason
# that matters: a demo that pulls :latest tests published code, not the branch
# you have checked out. It also removes a registry -- and a login -- from the
# path of getting the thing running on a laptop.
#
# The layers are shared with the control-plane build above, so this is much
# cheaper than three Rust builds after the first run.
IMAGE_REGISTRY="aether.local"
IMAGE_REPO="demo"
IMAGE_TAG="dev"

BUILD_LOG="$(mktemp -t aether-demo-build)"
for component in herald genesis operator; do
    image="${IMAGE_REGISTRY}/${IMAGE_REPO}/aether-${component}:${IMAGE_TAG}"
    note "building ${component}"
    # BuildKit writes its progress to stderr, so `>/dev/null` alone leaves a
    # thousand lines of cargo output in whatever is capturing this. Kept in a
    # file and printed only if the build fails, which is the only time anyone
    # wants it.
    if ! docker build --target "${component}" -t "${image}" . >"${BUILD_LOG}" 2>&1; then
        tail -30 "${BUILD_LOG}" >&2
        die "could not build the ${component} image (full log: ${BUILD_LOG})"
    fi
    k3d image import "${image}" --cluster "${CLUSTER}" >/dev/null 2>&1 \
        || die "could not import ${image} into ${CLUSTER}"
done
rm -f "${BUILD_LOG}"
note "three images imported into ${CLUSTER}"

# Kubernetes restarts a pod when its *spec* changes, and the tag does not move
# between runs. So a second run rebuilt three images, imported them, and left
# every pod on the binary from the first -- which reads as code that did not
# take effect. The restart below is what makes re-running this a redeploy.
RESTART_AFTER_INSTALL=1

# --------------------------------------------------------------- register the DP

step "registering the shared data plane"
# Idempotent by lookup rather than by an upsert the API does not offer: running
# this twice must not leave two data planes competing for the same cluster.
existing=$(curl -sS "${CONTROL_PLANE}/dataplanes" -H "Authorization: Bearer ${OPERATOR_TOKEN}" \
    | jq -r --arg r "${REGION}" \
        'first(.data[] | select(.region == $r and .allocation == "shared") | .id) // empty')

if [ -n "${existing}" ]; then
    DATAPLANE_ID="${existing}"
    note "reusing ${DATAPLANE_ID}"

    # Its secret is only readable in the answer that created it, and this run
    # did not create it. Issued again rather than looked up -- which is also
    # what an operator does when a cluster's secret leaks.
    response=$(curl -sS -X POST "${CONTROL_PLANE}/dataplanes/${DATAPLANE_ID}/credential" \
        -H "Authorization: Bearer ${OPERATOR_TOKEN}" -w $'\n%{http_code}')
    status="${response##*$'\n'}"
    [ "${status#2}" != "${status}" ] || die "could not issue a credential (HTTP ${status})"
    HERALD_CLIENT_ID=$(printf '%s' "${response%$'\n'*}" | jq -r '.herald_client_id // empty')
    HERALD_SECRET=$(printf '%s' "${response%$'\n'*}" | jq -r '.herald_secret // empty')
else
    response=$(curl -sS -X POST "${CONTROL_PLANE}/dataplanes" \
        -H "Authorization: Bearer ${OPERATOR_TOKEN}" \
        -H 'Content-Type: application/json' \
        -d "{\"mode\":\"shared\",\"region\":\"${REGION}\",\"capacity\":{\"cpu_millis\":8000,\"memory_mib\":16384,\"storage_gib\":200}}" \
        -w $'\n%{http_code}')
    status="${response##*$'\n'}"
    [ "${status#2}" != "${status}" ] || die "could not register a data plane (HTTP ${status})"
    DATAPLANE_ID=$(printf '%s' "${response%$'\n'*}" | jq -r '.id')
    HERALD_CLIENT_ID=$(printf '%s' "${response%$'\n'*}" | jq -r '.herald_client_id // empty')
    HERALD_SECRET=$(printf '%s' "${response%$'\n'*}" | jq -r '.herald_secret // empty')
    note "registered ${DATAPLANE_ID}"
fi

if [ -n "${HERALD_CLIENT_ID}" ]; then
    note "this cluster authenticates as ${HERALD_CLIENT_ID}"
else
    # An installation with no realm administrator configured mints nothing, and
    # the chart falls back to the client every cluster shares.
    note "no identity of its own: falling back to herald-service"
    HERALD_CLIENT_ID="herald-service"
    HERALD_SECRET="${HERALD_SECRET_SHARED}"
fi

# It starts in Provisioning and becomes Active on its first heartbeat, which
# Herald sends once the chart below is running. Nothing here forces it.

# ------------------------------------------------------------------- helm install

step "installing the data plane into ${CLUSTER}"
# A second release in the same namespace would run a second Herald claiming for
# a different data plane id, against the same cluster. Both would appear to
# work and each would see half the actions. Left to the operator to remove
# rather than deleted here -- this script does not uninstall things it did not
# install.
others=$(helm -n "${NAMESPACE}" list -q 2>/dev/null | grep -v "^${RELEASE}$" || true)
if [ -n "${others}" ]; then
    printf '\n\033[1;33m! another data plane release is installed in %s:\033[0m\n' "${NAMESPACE}"
    printf '%s\n' "${others}" | sed 's/^/    /'
    note "its Herald claims actions for whatever id it was installed with."
    note "Remove it unless you meant it:  helm -n ${NAMESPACE} uninstall <name>"
fi

# The chart does not create this one: the credentials are an installation's,
# not a chart's. Without it the operator refuses every instance that archives,
# saying this data plane has no object store credentials -- which is true, and
# is the whole difference between a demo that backs up and one that does not.
kubectl create namespace "${NAMESPACE}" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
kubectl -n "${NAMESPACE}" create secret generic aether-object-store \
    --from-literal=ACCESS_KEY_ID=aether \
    --from-literal=ACCESS_SECRET_KEY=aetheraether \
    --dry-run=client -o yaml | kubectl apply -f - >/dev/null
note "object store credentials in place"

# ------------------------------------------------------------------- local TLS

step "local TLS (mkcert)"
# A browser treats plain HTTP on anything but localhost/127.0.0.1 as an
# insecure context, which is what silently disables things like
# crypto.randomUUID -- a real, trusted certificate is what makes an instance
# behave in a laptop's browser the way it will in production's.
command -v mkcert >/dev/null 2>&1 || die "mkcert is required (brew install mkcert)"
mkcert -install >/dev/null 2>&1
CERT_DIR="$(mktemp -d)"
mkcert -cert-file "${CERT_DIR}/tls.crt" -key-file "${CERT_DIR}/tls.key" \
    "aether.local" "*.aether.local" >/dev/null 2>&1 \
    || die "mkcert could not issue a certificate for *.aether.local"
kubectl create namespace "${NAMESPACE}" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
kubectl -n "${NAMESPACE}" create secret tls aether-gateway-tls \
    --cert="${CERT_DIR}/tls.crt" --key="${CERT_DIR}/tls.key" \
    --dry-run=client -o yaml | kubectl apply -f - >/dev/null
rm -rf "${CERT_DIR}"
note "local CA trusted, *.aether.local certificate installed"

helm upgrade --install "${RELEASE}" charts/aether-dataplane \
    --namespace "${NAMESPACE}" --create-namespace \
    --set "objectStore.enabled=true" \
    --set "objectStore.endpoint=http://host.k3d.internal:${RUSTFS_PORT}" \
    --set "image.registry=${IMAGE_REGISTRY}" \
    --set "image.repository=${IMAGE_REPO}" \
    --set "image.tag=${IMAGE_TAG}" \
    --set "image.pullPolicy=Never" \
    --set "dataplane.id=${DATAPLANE_ID}" \
    --set "controlPlane.url=http://host.k3d.internal:${AETHER_API_PORT}" \
    --set "controlPlane.auth.issuer=http://host.k3d.internal:${FERRISKEY_API_PORT}/realms/aether" \
    --set "controlPlane.auth.clientId=${HERALD_CLIENT_ID}" \
    --set "controlPlane.auth.clientSecret=${HERALD_SECRET}" \
    --set "herald.logIndex.url=http://host.k3d.internal:${QUICKWIT_PORT}" \
    --set "herald.otlp.enabled=true" \
    --set "gateway.tls.secretName=aether-gateway-tls" \
    --wait --timeout 5m 2>&1 | tail -4 || die "helm install failed"

# ------------------------------------------------------------------- new images

if [ "${RESTART_AFTER_INSTALL:-0}" = "1" ]; then
    step "restarting the data plane onto the images just built"
    kubectl -n "${NAMESPACE}" rollout restart deployment \
        -l app.kubernetes.io/part-of=aether >/dev/null
    kubectl -n "${NAMESPACE}" rollout status deployment \
        -l app.kubernetes.io/part-of=aether --timeout=3m >/dev/null \
        || note "some pods are still coming up; see kubectl -n ${NAMESPACE} get pods"
    note "running the code in this working tree"
fi

# ------------------------------------------------------------------ wait for life

step "waiting for the data plane to report"
# Freshly obtained. The token taken before the images were built is minutes old
# by now, and reading `.status` off a 401 body printed "the data plane is still
# 401" over a stack that was working -- a false alarm at the exact moment
# somebody trusts this script.
OPERATOR_TOKEN=$(token aether-operator-cli "${OPERATOR_SECRET}")

# The heartbeat is what promotes it out of Provisioning: it is the only evidence
# the control plane gets that Herald is running inside the cluster.
for _ in $(seq 60); do
    status=$(curl -sS "${CONTROL_PLANE}/dataplanes/${DATAPLANE_ID}" \
        -H "Authorization: Bearer ${OPERATOR_TOKEN}" | jq -r '.data.status // .status')
    [ "${status}" = "active" ] && break
    sleep 5
done

if [ "${status}" != "active" ]; then
    printf '\n\033[1;33m! the data plane is still %s\033[0m\n' "${status}"
    note "Herald has not reported yet. Its logs:"
    note "  KUBECONFIG=${KUBECONFIG_PATH} kubectl -n ${NAMESPACE} logs -l app.kubernetes.io/component=herald --tail=50"
    note "Everything else is up; a deployment created now will sit in Pending."
fi

cat <<SUMMARY

$( [ "${status}" = "active" ] && printf '\033[1;32m✅ ready\033[0m' || printf '\033[1;33m⚠ up, data plane not reporting\033[0m' )

   data plane   ${DATAPLANE_ID}  (${REGION}, ${status})
   control API  ${CONTROL_PLANE}
   identity     ${ISSUER}

   Start the console:

     cd apps/console
     printf 'VITE_API_URL=%s\nVITE_OIDC_ISSUER_URL=%s\nVITE_OIDC_CLIENT_ID=console\n' \\
       '${CONTROL_PLANE}' '${ISSUER}' > .env
     pnpm install && pnpm dev -- --port ${CONSOLE_PORT} --host 127.0.0.1

   Open http://localhost:${CONSOLE_PORT} and **create an account** on the login page.

   The port matters: it is the one the realm allows a redirect to. 'pnpm dev'
   uses --strictPort so it fails rather than quietly moving to the next free
   port, which would fail later at the identity provider instead. If ${CONSOLE_PORT}
   is taken, re-run with CONSOLE_PORT=5175 and the realm follows.

   Self-registration is on for this realm, and it is the only way in: FerrisKey
   has no admin API for setting another user's password, so an account created
   by Terraform would exist and be unable to log in. Turned off for anything
   that is not a throwaway stack.

   Then create a deployment and watch it land:

     Data planes -> ${DATAPLANE_ID}

   A shared deployment is placed on the data plane above. A dedicated one
   provisions a data plane of its own for the organisation -- against the same
   k3d cluster locally, which is what the local provisioner is for.

   Once a FerrisKey deployment is Running and you have sent it some traffic,
   its logs and traces are searchable from the deployment's own Logs and
   Traces tabs -- Herald ships both to the Quickwit this script just pointed
   it at (http://host.k3d.internal:${QUICKWIT_PORT} from inside the cluster,
   http://localhost:${QUICKWIT_PORT} from here).

   Tear down:  ./scripts/demo.sh down

SUMMARY
