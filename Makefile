init:
	@echo "Installing git hooks (pre-commit) and running initial checks"
	pre-commit install || echo "pre-commit not installed (pip install pre-commit)"
	cargo fmt --all
	cargo clippy --all-targets --all-features -- -D warnings || true
	cargo check

run:
	cargo run

# --- Development Workflow Targets (see docs/dev-workflow.md) ---

# Start only infrastructure dependencies for local dev (datastores, object storage)
dev-infra:
	@echo "[dev-infra] Starting Postgres, Redis, MinIO containers"
	docker compose up -d postgres redis minio

# Alias retained for backward compatibility
docker-dev: dev-infra

# Run backend with auto-reload (requires `cargo install cargo-watch` once)
dev-backend:
	@command -v cargo-watch >/dev/null 2>&1 || { echo "cargo-watch not installed. Run: cargo install cargo-watch"; exit 1; }
	RUST_LOG=$${RUST_LOG:-info} cargo watch -x 'run'

# Run frontend (Vite dev server)
dev-frontend:
	cd rib-react && npm install --no-audit --no-fund && npm run dev

# Build all service images
build-images:
	docker compose build

# Execute smoke test script (creates it if missing with instructions)
smoke:
	@if [ ! -x scripts/smoke.sh ]; then \
		mkdir -p scripts; \
		echo "Smoke script missing. Creating a template at scripts/smoke.sh"; \
		echo '#!/usr/bin/env bash' > scripts/smoke.sh; \
		echo 'echo "(placeholder) customize smoke tests"' >> scripts/smoke.sh; \
		chmod +x scripts/smoke.sh; \
	fi
	./scripts/smoke.sh

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Docker commands
docker-validate:
	@./validate-docker-setup.sh

docker-up:
	docker compose up -d

docker-down:
	docker compose down

docker-down-volumes:
	docker compose down -v

docker-logs:
	docker compose logs -f

docker-status:
	docker compose ps

docker-dev:
	@echo "(deprecated) use make dev-infra"

up:
	docker compose up -d

down:
	docker compose down -v
