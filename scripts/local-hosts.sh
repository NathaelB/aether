#!/usr/bin/env bash
# Makes the instances running on the local cluster reachable by name.
#
# Everything already works through the Gateway: `curl -H "Host: <name>"
# http://localhost:8081` answers. What is missing is only that a browser has no
# way to resolve `<name>`, and /etc/hosts has no wildcards, so every deployment
# needs a line of its own.
#
# The hostnames are read from the routes actually serving, not guessed from a
# naming convention. A convention drifts; a route is what Envoy is matching on
# right now.
set -euo pipefail

CLUSTER_NAME="${AETHER_CLUSTER_NAME:-aether-local}"
CONTEXT="k3d-${CLUSTER_NAME}"
# Overridable so the apply path can be exercised without touching the real one.
HOSTS_FILE="${AETHER_HOSTS_FILE:-/etc/hosts}"

# Everything between these two lines belongs to this script. Nothing outside
# them is ever read, moved or rewritten: /etc/hosts is a file other things care
# about, and a tool that reformats it is a tool nobody runs twice.
BEGIN_MARKER="# >>> aether ${CLUSTER_NAME} >>>"
END_MARKER="# <<< aether ${CLUSTER_NAME} <<<"

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "❌ $1 is required but not installed" >&2
        exit 1
    }
}

usage() {
    cat <<USAGE
usage: $(basename "$0") <print|apply|remove>

  print   show the lines to add, and the URLs they make work
  apply   write them into ${HOSTS_FILE} (needs sudo)
  remove  take them back out again

Environment:
  AETHER_CLUSTER_NAME   k3d cluster to read routes from (default: ${CLUSTER_NAME})
  AETHER_HOSTS_FILE     file to write (default: /etc/hosts)
USAGE
}

# The host port k3d publishes for the node's port 80.
#
# Read from the running container rather than assumed: the cluster script lets
# it be overridden on creation, and a URL printed with the wrong port is worse
# than no URL at all.
http_port() {
    docker port "k3d-${CLUSTER_NAME}-serverlb" 80/tcp 2>/dev/null \
        | head -1 \
        | sed 's/.*://' \
        || true
}

# Fails loudly when the cluster is not there.
#
# Without this the pipeline below simply produces nothing under `set -o
# pipefail`, the script exits before printing anything, and the answer to "why
# did nothing happen" is silence.
reachable() {
    kubectl --context "${CONTEXT}" get --raw /readyz >/dev/null 2>&1
}

hostnames() {
    local routes
    routes="$(kubectl --context "${CONTEXT}" get httproute -A \
        -o jsonpath='{range .items[*]}{range .spec.hostnames[*]}{@}{"\n"}{end}{end}' 2>/dev/null || true)"

    printf '%s\n' "${routes}" | sed '/^$/d' | sort -u
}

block() {
    local names="$1"

    echo "${BEGIN_MARKER}"
    echo "# Written by scripts/local-hosts.sh. Everything between the markers is"
    echo "# rewritten wholesale; edits inside are lost, edits outside are kept."
    while IFS= read -r name; do
        [ -n "${name}" ] && echo "127.0.0.1 ${name}"
    done <<<"${names}"
    echo "${END_MARKER}"
}

# The file with any previous block of ours taken out.
#
# sed rather than grep -v on the hostnames: removing by name would also remove
# a line somebody wrote by hand for the same host, which is theirs and not ours.
without_block() {
    sed "/^${BEGIN_MARKER}$/,/^${END_MARKER}$/d" "${HOSTS_FILE}"
}

# Names already resolved by a line somebody wrote themselves.
#
# Reported, never removed. A duplicate is harmless -- the addresses are the same
# and the first match wins -- and a line outside our markers belongs to whoever
# put it there. Silently deleting it is how a tool stops being one anybody runs.
warn_about_duplicates() {
    local names="$1"
    local existing=""

    while IFS= read -r name; do
        [ -z "${name}" ] && continue
        if without_block | grep -qE "^[^#]*[[:space:]]${name}([[:space:]]|$)"; then
            existing="${existing}  ${name}"$'\n'
        fi
    done <<<"${names}"

    if [ -n "${existing}" ]; then
        cat <<MESSAGE
ℹ️  already resolved by a line of your own, outside the markers:
${existing}
Harmless: the address is the same either way. Left alone because that line is
yours, not this script's.
MESSAGE
    fi
}

no_routes() {
    cat <<MESSAGE
No instance is served on ${CLUSTER_NAME} yet, so there is nothing to add.

Create one first -- \`make demo\` brings up the whole stack, or apply an example:

  kubectl --context ${CONTEXT} apply -f k8s/examples/identity-instance-ferriskey.yaml
MESSAGE
}

main() {
    require kubectl
    require docker

    local command="${1:-print}"
    case "${command}" in
        print | apply | remove) ;;
        -h | --help | help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac

    if [ "${command}" = "remove" ]; then
        if ! grep -qF "${BEGIN_MARKER}" "${HOSTS_FILE}" 2>/dev/null; then
            echo "nothing to remove: ${HOSTS_FILE} has no ${CLUSTER_NAME} block"
            exit 0
        fi
        without_block >"${HOSTS_FILE}.aether.tmp"
        cat "${HOSTS_FILE}.aether.tmp" >"${HOSTS_FILE}"
        rm -f "${HOSTS_FILE}.aether.tmp"
        echo "✅ removed the ${CLUSTER_NAME} block from ${HOSTS_FILE}"
        exit 0
    fi

    if ! reachable; then
        cat >&2 <<MESSAGE
❌ cannot reach the cluster ${CLUSTER_NAME} (context ${CONTEXT}).

  make local-up          create it
  k3d cluster list       see what is there
MESSAGE
        exit 1
    fi

    local names
    names="$(hostnames)"
    if [ -z "${names}" ]; then
        no_routes
        exit 0
    fi

    local port
    port="$(http_port)"
    if [ -z "${port}" ]; then
        echo "❌ the cluster ${CLUSTER_NAME} is not running, or publishes no port for 80" >&2
        exit 1
    fi

    if [ "${command}" = "print" ]; then
        block "${names}"
        echo
        echo "Then, on port ${port}:"
        while IFS= read -r name; do
            [ -n "${name}" ] && echo "  http://${name}:${port}"
        done <<<"${names}"
        echo
        echo "To write them: sudo make local-hosts-apply"
        exit 0
    fi

    # Rewritten wholesale rather than appended to, so a deployment that is gone
    # stops resolving instead of pointing at nothing for ever.
    {
        without_block
        block "${names}"
    } >"${HOSTS_FILE}.aether.tmp"
    cat "${HOSTS_FILE}.aether.tmp" >"${HOSTS_FILE}"
    rm -f "${HOSTS_FILE}.aether.tmp"

    warn_about_duplicates "${names}"

    echo "✅ ${HOSTS_FILE} now resolves:"
    while IFS= read -r name; do
        [ -n "${name}" ] && echo "  http://${name}:${port}"
    done <<<"${names}"
}

main "$@"
