# Backend task runner. This Makefile is the single entry point for every backend command.
# Run `make` or `make help` to list targets. Full guideline: docs/deployment.md
#
# Conventions
#   ENV      environment name: dev | staging | prod   (cf-* and create-user targets)
#   SERVICE  admin | client                           (cf-secret)
#   NAME     secret name                              (cf-secret)
# Targets marked [CLOUD] change real Cloudflare state; they confirm first.

.DEFAULT_GOAL := help
SHELL := /bin/bash

.PHONY: help check test build-admin build-client dev-admin dev-client \
	test-d1 migrate-local migrate-staging migrate-prod create-user \
	cf-whoami cf-bootstrap cf-secret cf-dry-run deploy-staging deploy-prod

help: ## List all targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z0-9_-]+:.*## / {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# ---- Verify & build ---------------------------------------------------------

check: ## Type-check Workers for wasm32
	cargo check --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown

test: ## Run all tests
	cargo test --workspace

# D1-level tests: real SQL on isolated local D1 state under target/tmp/d1 (never the dev DB, never remote).
# Needs Node.js and wrangler (npx). Tests are #[ignore]d in `make test`; one thread keeps memory low.
# Plan p2 adds the prerequisite line `test-d1: test-d1-read` (client SQL on the same harness) without editing this recipe.
test-d1: ## Run D1-level SQL tests on local D1 via wrangler (A2, scenario 27, client SQL)
	cargo test -p seed-build --test d1 -- --ignored --test-threads=1

build-admin: ## Build the admin Worker bundle
	cd admin && worker-build --release

build-client: ## Build the client Worker bundle
	cd client && worker-build --release

# ---- Local dev --------------------------------------------------------------

dev-admin: ## Run admin Worker locally (wrangler dev)
	cd admin && npx wrangler dev

dev-client: ## Run client Worker locally (wrangler dev)
	cd client && npx wrangler dev

# Migrations are shared (../migrations) and applied through the admin worker's D1 binding.
migrate-local: ## Apply D1 migrations to local state
	cd admin && npx wrangler d1 migrations apply DB --local

migrate-staging: ## [CLOUD] Apply D1 migrations to staging
	cd admin && npx wrangler d1 migrations apply DB --env staging --remote

migrate-prod: ## [CLOUD] Apply D1 migrations to prod
	cd admin && npx wrangler d1 migrations apply DB --env prod --remote

# Runs from admin/: the CLI reads admin/.dev.vars (dev pepper) and calls `wrangler d1 execute DB`, which needs admin/wrangler.toml.
create-user: ## Create a user. Usage: make create-user EMAIL=a@b.com ROLE=owner ENV=dev
	cd admin && cargo run --manifest-path ../tools/create-user/Cargo.toml -- --email=$(EMAIL) --role=$(ROLE) --env=$(or $(ENV),dev)

# ---- Cloudflare setup & deploy ---------------------------------------------

cf-whoami: ## Show the logged-in Cloudflare account (login first: npx wrangler login)
	cd admin && npx wrangler whoami

cf-bootstrap: ## [CLOUD] One-time: create D1 + R2 for ENV=staging|prod
	@case "$(ENV)" in staging|prod) ;; *) echo "usage: make cf-bootstrap ENV=staging|prod"; exit 2;; esac
	@cd admin && npx wrangler whoami
	@read -r -p "Create D1 + R2 resources for '$(ENV)' in the account above? [y/N] " ok; \
		[[ "$$ok" == "y" || "$$ok" == "Y" ]] || { echo aborted; exit 1; }
	cd admin && npx wrangler d1 create maemorkratai-db-$(ENV)
	cd admin && npx wrangler r2 bucket create maemorkratai-public-$(ENV)
	cd admin && npx wrangler r2 bucket create maemorkratai-private-$(ENV)
	@echo
	@echo "Next: paste the printed D1 database_id into admin/wrangler.toml and client/wrangler.toml (env.$(ENV)),"
	@echo "then: make cf-secret (see docs/deployment.md), make migrate-$(ENV), make deploy-$(ENV)."

cf-secret: ## [CLOUD] Set a Worker secret. Usage: make cf-secret SERVICE=admin NAME=PASSWORD_PEPPER ENV=staging
	@case "$(SERVICE)" in admin|client) ;; *) echo "usage: make cf-secret SERVICE=admin|client NAME=<SECRET> ENV=staging|prod"; exit 2;; esac
	@test -n "$(NAME)" || { echo "NAME is required"; exit 2; }
	@case "$(ENV)" in staging|prod) ;; *) echo "ENV must be staging or prod"; exit 2;; esac
	cd $(SERVICE) && npx wrangler secret put $(NAME) --env $(ENV)

cf-dry-run: ## Bundle both Workers for ENV=staging|prod without deploying (needs no changes in Cloudflare)
	@case "$(ENV)" in staging|prod) ;; *) echo "usage: make cf-dry-run ENV=staging|prod"; exit 2;; esac
	cd admin && npx wrangler deploy --dry-run --env $(ENV) --outdir ../.dry-run/admin-$(ENV)
	cd client && npx wrangler deploy --dry-run --env $(ENV) --outdir ../.dry-run/client-$(ENV)

# Manual deploys (CI uses .github/workflows/backend-deploy.yml). Admin first: it serves the JWKS.
deploy-staging: ## [CLOUD] Deploy admin then client to staging
	cd admin && npx wrangler deploy --env staging
	cd client && npx wrangler deploy --env staging

deploy-prod: ## [CLOUD] Deploy admin then client to prod
	cd admin && npx wrangler deploy --env prod
	cd client && npx wrangler deploy --env prod
