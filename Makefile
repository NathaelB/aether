.PHONY: help crds install-crds uninstall-crds verify-crds test build local-up local-down local-status

help: ## Afficher l'aide
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# === Build ===

build: ## Compiler tout le workspace
	cargo build --workspace

test: ## Lancer les tests
	cargo nextest run

test-crds: ## Tester seulement les CRDs
	cargo test -p aether-crds

# === CRDs ===

crds: ## Générer les CRDs
	@./scripts/generate-crds.sh

install-crds: crds ## Générer et installer les CRDs dans le cluster
	@echo "📦 Installing CRDs in Kubernetes cluster..."
	@kubectl apply -f k8s/crds/
	@echo "✅ CRDs installed successfully"
	@echo ""
	@kubectl get crd | grep aether.io

uninstall-crds: ## Désinstaller les CRDs du cluster
	@echo "🗑️  Uninstalling CRDs..."
	@kubectl delete -f k8s/crds/ --ignore-not-found
	@echo "✅ CRDs uninstalled"

verify-crds: ## Vérifier les CRDs installées
	@echo "🔍 Verifying CRDs..."
	@kubectl get crd | grep aether.io || echo "❌ No Aether CRDs found"


# === Cluster local ===

local-up: ## Créer le cluster k3d dédié à Aether et y installer CRDs + CloudNativePG
	@./scripts/local-cluster.sh up

local-down: ## Supprimer le cluster k3d dédié
	@./scripts/local-cluster.sh down

local-status: ## Afficher ce qui est installé sur le cluster local
	@./scripts/local-cluster.sh status

# === Cleanup ===

clean: ## Nettoyer les artifacts de build
	cargo clean
	rm -rf k8s/crds/*.yaml
