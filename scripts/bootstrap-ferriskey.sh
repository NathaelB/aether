#!/usr/bin/env bash
# Creates the realm and the OIDC clients Aether needs in a local Ferriskey.
#
# Nothing in the repository provisioned these, so a fresh stack came up with a
# control plane that could not validate a token and a Herald that could not
# obtain one -- both failing in ways that look like bugs rather than like
# missing setup.
#
# Idempotent: an existing realm or client is left alone, so this can be re-run
# after a restart without producing duplicates.
set -euo pipefail

FERRISKEY_URL="${FERRISKEY_URL:-http://localhost:3334}"
REALM="${AETHER_REALM:-aether}"
ADMIN_USER="${FERRISKEY_ADMIN_USER:-admin}"
ADMIN_PASSWORD="${FERRISKEY_ADMIN_PASSWORD:-admin}"
CONSOLE_REDIRECT="${CONSOLE_REDIRECT_URI:-http://localhost:5173}"

require() {
    command -v "$1" >/dev/null 2>&1 || { echo "❌ $1 is required" >&2; exit 1; }
}
require curl
require jq

# Prints the body and returns non-zero on a non-2xx status. curl exits 0 on a
# 401 without -f, and trusting that is how "realm already exists" ended up
# meaning "the realm does not exist and every later call failed".
api() {
    local method="$1" path="$2"
    shift 2
    local body status
    body=$(curl -sS -X "${method}" "${FERRISKEY_URL}${path}" \
        -H "Authorization: Bearer ${TOKEN}" \
        -H "Content-Type: application/json" \
        -w $'\n%{http_code}' "$@")
    status="${body##*$'\n'}"
    body="${body%$'\n'*}"

    printf '%s' "${body}"
    case "${status}" in
        2*) return 0 ;;
        *) return 1 ;;
    esac
}

echo "🔑 authenticating against ${FERRISKEY_URL}"
# The password grant on master is how an operator bootstraps; the clients this
# script creates use client credentials instead.
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

echo "🏛  realm ${REALM}"
if api GET "/realms/${REALM}" >/dev/null 2>&1; then
    echo "   already exists"
else
    api POST "/realms" -d "$(jq -nc --arg n "${REALM}" '{name: $n, display_name: "Aether"}')" >/dev/null
    echo "   created"
fi

# Returns the internal id of a client, creating it first if it is absent.
ensure_client() {
    local client_id="$1" body="$2"
    local existing
    existing=$(api GET "/realms/${REALM}/clients" | jq -r \
        --arg cid "${client_id}" '(.data // .) | if type == "array" then .[] else empty end
                                  | select(.client_id == $cid) | .id' | head -1)

    if [ -n "${existing}" ] && [ "${existing}" != "null" ]; then
        echo "   ${client_id}: already exists" >&2
        echo "${existing}"
        return
    fi

    local created
    created=$(api POST "/realms/${REALM}/clients" -d "${body}")
    echo "   ${client_id}: created" >&2
    echo "${created}" | jq -r '.data.id // .id'
}

echo "👤 clients"
# The console is a public client: a browser cannot keep a secret.
ensure_client "console" "$(jq -nc '{
    client_id: "console",
    name: "Aether Console",
    client_type: "public",
    public_client: true,
    enabled: true,
    protocol: "openid-connect",
    service_account_enabled: false,
    direct_access_grants_enabled: true
}')" >/dev/null

# Herald is confidential with a service account: it authenticates as itself,
# with no user involved. The control plane rejects any caller whose client id
# does not contain "herald-service".
HERALD_ID=$(ensure_client "herald-service" "$(jq -nc '{
    client_id: "herald-service",
    name: "Herald",
    client_type: "confidential",
    public_client: false,
    enabled: true,
    protocol: "openid-connect",
    service_account_enabled: true,
    direct_access_grants_enabled: false
}')")

SECRET=$(api GET "/realms/${REALM}/clients/${HERALD_ID}/client-secret" \
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

   The secret is printed rather than written to a file: it belongs in whatever
   holds your secrets, not in the working tree next to a .gitignore that may or
   may not cover it. Redirect URIs for the console still have to be added --
   ${CONSOLE_REDIRECT} is the usual one.
SUMMARY
