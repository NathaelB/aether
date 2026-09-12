#!/bin/bash

set -e

cd "$(dirname "$0")/.."

echo "🚀 Generating Aether CRDs..."

mkdir -p k8s/crds

# Générer la CRD IdentityInstance depuis la lib
echo "📝 Generating IdentityInstance CRD..."
cargo run --quiet -p aether-crds --example generate_crd -- identity-instance > k8s/crds/identity-instance.yaml

echo "📝 Generating IdentityInstanceUpgrade CRD..."
cargo run --quiet -p aether-crds --example generate_crd -- identity-instance-upgrade > k8s/crds/identity-instance-upgrade.yaml

echo "📝 Generating IdentityInstanceBackup CRDs..."
cargo run --quiet -p aether-crds --example generate_crd -- identity-instance-backup > k8s/crds/identity-instance-backup.yaml

echo "✅ CRDs generated successfully:"
echo "  - k8s/crds/identity-instance.yaml"
echo "  - k8s/crds/identity-instance-upgrade.yaml"
echo "  - k8s/crds/identity-instance-backup.yaml"
echo ""
echo "To install in your cluster, run:"
echo "  kubectl apply -f k8s/crds/"
