#!/usr/bin/env bash
# Applies deploy/ferriskey/terraform to a local FerrisKey and prints the values
# the control plane, Herald and the console need.
#
# The realm is declared with FerrisKey's own Terraform provider rather than
# driven through the API by a script: it stays in step with the API, it manages
# redirect URIs, and it converges. `ferris-ctl realm import` was the other
# candidate and does not converge -- it creates, and returns a 500 on a realm
# that already exists.
set -euo pipefail

FERRISKEY_URL="${FERRISKEY_URL:-http://localhost:3334}"
ADMIN_USER="${FERRISKEY_ADMIN_USER:-admin}"
ADMIN_PASSWORD="${FERRISKEY_ADMIN_PASSWORD:-admin}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TF_DIR="${REPO_ROOT}/deploy/ferriskey/terraform"

require() {
    command -v "$1" >/dev/null 2>&1 || { echo "❌ $1 is required" >&2; exit 1; }
}
require terraform
require curl
require jq

echo "🏛  applying ${TF_DIR}"
terraform -chdir="${TF_DIR}" init -input=false -no-color >/dev/null
TF_VAR_ferriskey_url="${FERRISKEY_URL}" \
TF_VAR_admin_username="${ADMIN_USER}" \
TF_VAR_admin_password="${ADMIN_PASSWORD}" \
    terraform -chdir="${TF_DIR}" apply -auto-approve -input=false -no-color \
    | tail -1

ISSUER=$(terraform -chdir="${TF_DIR}" output -raw issuer)
HERALD_UUID=$(terraform -chdir="${TF_DIR}" output -raw herald_client_uuid)

# The provider stores "***" for the secret at v0.1.0 -- the API masks it in the
# client response and the provider records the mask. So the real one is read
# from the endpoint that returns it. Drop this once the provider is fixed and
# take `terraform output -raw herald_client_secret` instead.
TOKEN=$(curl -sS -X POST \
    "${FERRISKEY_URL}/realms/master/protocol/openid-connect/token" \
    -d grant_type=password -d client_id=admin-cli \
    -d "username=${ADMIN_USER}" -d "password=${ADMIN_PASSWORD}" \
    | jq -r '.access_token')

if [ -z "${TOKEN}" ] || [ "${TOKEN}" = "null" ]; then
    echo "❌ could not authenticate as ${ADMIN_USER}." >&2
    exit 1
fi

# Checks the status, not curl's exit code: curl exits 0 on a 401 without -f,
# and trusting it once already turned a missing realm into a reported success.
response=$(curl -sS "${FERRISKEY_URL}/realms/aether/clients/${HERALD_UUID}/client-secret" \
    -H "Authorization: Bearer ${TOKEN}" -w $'\n%{http_code}')
status="${response##*$'\n'}"
if [ "${status#2}" = "${status}" ]; then
    echo "❌ could not read the herald-service secret (HTTP ${status})" >&2
    exit 1
fi
SECRET=$(printf '%s' "${response%$'\n'*}" | jq -r '.data.client_secret // .client_secret')

cat <<SUMMARY

✅ realm ready

   Control plane:
     AUTH_ISSUER=${ISSUER}

   Herald:
     AUTH_ISSUER=${ISSUER}
     AUTH_CLIENT_ID=herald-service
     AUTH_CLIENT_SECRET=${SECRET}

   Console (apps/console/.env):
     VITE_OIDC_ISSUER_URL=${ISSUER}
     VITE_OIDC_CLIENT_ID=console

   The secret is printed rather than written: it belongs in whatever holds your
   secrets, not in the working tree.

   Preview a change to the realm:
     terraform -chdir=deploy/ferriskey/terraform plan
SUMMARY
