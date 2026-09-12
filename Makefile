.PHONY: help crds install-crds uninstall-crds verify-crds test test-objectstore build local-up local-down local-status bootstrap-auth demo demo-down

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


# === Démo complète ===

demo: ## Monter tout Aether en local, prêt à créer un déploiement depuis la console
	@./scripts/demo.sh up

demo-down: ## Tout supprimer : compose, volumes et cluster k3d
	@./scripts/demo.sh down

# === Identité ===

bootstrap-auth: ## Créer le realm et les clients OIDC dans un Ferriskey local
	@./scripts/bootstrap-ferriskey.sh

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

test-objectstore: ## Lancer les tests qui ont besoin d'un vrai object store
	@test -n "$$OBJECT_STORE_ENDPOINT" || { \
		echo "OBJECT_STORE_ENDPOINT n'est pas défini : les tests se skipperaient en silence."; \
		echo "Voir .env.example — le port doit être celui publié par docker-compose (9800)."; \
		exit 1; \
	}
	cargo test -p aether-s3 --test object_store
	cargo test -p aether-api --test archive_bucket

test-integration: ## Lancer les tests qui ont besoin d'un vrai Postgres
	@test -n "$$DATABASE_URL" || { \
		echo "DATABASE_URL n'est pas défini : les tests d'intégration se skipperaient en silence."; \
		echo "Voir .env.example — le port doit être celui publié par docker-compose."; \
		exit 1; \
	}
	cargo test -p aether-postgres --tests
