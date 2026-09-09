#!/usr/bin/env bash
# Applies deploy/ferriskey/aether-realm.yaml to a local FerrisKey, then prints
# Herald's client secret.
#
# The realm itself is applied with `ferris-ctl`, FerrisKey's own CLI: it takes a
# declarative description, supports a --dry-run, and handles redirect URIs and
# roles that a script driving the REST API by hand would have to reimplement and
# then keep in step with the API.
#
# Re-runnable, but not because the import is: `ferris-ctl realm import` creates
# and fails with a 500 on a realm that already exists, so this checks first and
# skips. Making the import itself converge belongs upstream in FerrisKey.
set -euo pipefail

FERRISKEY_URL="${FERRISKEY_URL:-http://localhost:3334}"
REALM="${AETHER_REALM:-aether}"
ADMIN_USER="${FERRISKEY_ADMIN_USER:-admin}"
ADMIN_PASSWORD="${FERRISKEY_ADMIN_PASSWORD:-admin}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REALM_FILE="${REALM_FILE:-${REPO_ROOT}/deploy/ferriskey/aether-realm.yaml}"

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "❌ $1 is required." >&2
        [ "$1" = "ferris-ctl" ] && echo "   cargo install --path <ferriskey>/cli" >&2
        exit 1
    }
}
require ferris-ctl
require curl
require jq

# The realm's discovery document answers without a token, which makes it the
# cheapest way to ask whether the realm is there.
realm_exists() {
    curl -sS -o /dev/null -w '%{http_code}' \
        "${FERRISKEY_URL}/realms/${REALM}/.well-known/openid-configuration" \
        | grep -q '^2'
}

if realm_exists; then
    echo "🏛  realm ${REALM} already exists, not re-importing"
    echo "   To apply a change to ${REALM_FILE##*/}, delete the realm first, or"
    echo "   apply the difference with ferris-ctl client/user commands."
else
    echo "🏛  applying ${REALM_FILE}"
    FERRISKEY_URL="${FERRISKEY_URL}" ferris-ctl realm import \
        --from config --file "${REALM_FILE}" -o yaml
fi

# Reading a secret needs a session, which bootstrapping does not have yet:
# ferris-ctl authenticates with a client id and secret, and the only client that
# exists before this ran is the public admin-cli, which has none. So this one
# call goes through the API with an admin token.
echo
echo "🔑 reading the herald-service secret"
TOKEN=$(curl -sS -X POST \
    "${FERRISKEY_URL}/realms/master/protocol/openid-connect/token" \
    -d grant_type=password \
    -d client_id=admin-cli \
    -d "username=${ADMIN_USER}" \
    -d "password=${ADMIN_PASSWORD}" | jq -r '.access_token')

if [ -z "${TOKEN}" ] || [ "${TOKEN}" = "null" ]; then
    echo "❌ could not authenticate as ${ADMIN_USER}." >&2
    echo "   Override with FERRISKEY_ADMIN_USER / FERRISKEY_ADMIN_PASSWORD." >&2
    exit 1
fi

# Checks the status rather than curl's exit code, which is 0 on a 401 without
# -f -- trusting it once already turned "the realm is missing" into a success.
fetch() {
    local response status
    response=$(curl -sS "${FERRISKEY_URL}$1" \
        -H "Authorization: Bearer ${TOKEN}" -w $'\n%{http_code}')
    status="${response##*$'\n'}"
    printf '%s' "${response%$'\n'*}"
    [ "${status#2}" != "${status}" ]
}

HERALD_ID=$(fetch "/realms/${REALM}/clients" \
    | jq -r '(.data // .) | if type == "array" then .[] else empty end
             | select(.client_id == "herald-service") | .id' | head -1)

if [ -z "${HERALD_ID}" ]; then
    echo "❌ herald-service was not found in realm ${REALM}" >&2
    exit 1
fi

SECRET=$(fetch "/realms/${REALM}/clients/${HERALD_ID}/client-secret" \
    | jq -r '.data.client_secret // .client_secret')

cat <<SUMMARY

✅ ${REALM} is ready

   Control plane:
     AUTH_ISSUER=${FERRISKEY_URL}/realms/${REALM}

   Herald:
     AUTH_ISSUER=${FERRISKEY_URL}/realms/${REALM}
     AUTH_CLIENT_ID=herald-service
     AUTH_CLIENT_SECRET=${SECRET}

   Console (apps/console/.env):
     VITE_OIDC_ISSUER_URL=${FERRISKEY_URL}/realms/${REALM}
     VITE_OIDC_CLIENT_ID=console

   The secret is printed rather than written: it belongs in whatever holds your
   secrets, not in the working tree next to a .gitignore that may or may not
   cover it.

   Preview a change to the realm without touching the server:
     ferris-ctl realm import --from config --file ${REALM_FILE} --dry-run -o yaml
SUMMARY
