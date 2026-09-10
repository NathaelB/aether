#!/usr/bin/env bash
# Regenerates the console's API client from the Rust handlers.
#
# `apps/console/src/api/api.client.ts` and `api.tanstack.ts` are generated
# files. Nobody edits them by hand -- when the API changes, this is what makes
# the console see the change. Before this script existed the client had drifted
# far enough that `CreateDeploymentRequest` carried five of the API's ten
# fields, and the console silently dropped everything it could not name.
#
# The OpenAPI document is committed too, at the repository root: it is the
# contract between the two halves, and a diff on it is the readable half of a
# regeneration that otherwise moves hundreds of lines of generated TypeScript.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONSOLE="${REPO_ROOT}/apps/console"
DOCUMENT="${REPO_ROOT}/openapi.json"

# The console's toolchain is pinned by `packageManager` in its package.json;
# going through corepack is what honours it. pnpm 11 ignores the `pnpm.overrides`
# block this project relies on for rolldown-vite, so an unpinned pnpm fails on
# `--frozen-lockfile` with ERR_PNPM_LOCKFILE_CONFIG_MISMATCH.
pnpm() { (cd "${CONSOLE}" && CI=true corepack pnpm "$@"); }

echo "📜 dumping the OpenAPI document"
# No database, no server, no port: the document is a pure function of the
# handler annotations. SQLX_OFFLINE keeps the query cache from wanting one.
SQLX_OFFLINE=true cargo run -q -p aether-api --example dump-openapi > "${DOCUMENT}"

echo "⚙️  generating the client"
pnpm exec typed-openapi "${DOCUMENT}" \
    --output src/api/api.client.ts \
    --tanstack api.tanstack.ts

echo "🧹 formatting"
pnpm exec prettier --write src/api/api.client.ts src/api/api.tanstack.ts >/dev/null

echo "🔎 type-checking"
pnpm exec tsc -b

cat <<SUMMARY

✅ client regenerated

   $(git -C "${REPO_ROOT}" diff --stat -- openapi.json apps/console/src/api | tail -1)

   Review the diff on openapi.json first -- it is the part that says what
   actually changed about the API.
SUMMARY
