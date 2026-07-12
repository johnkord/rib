.PHONY: init run dev-infra docker-dev dev-backend dev-frontend build-images smoke fmt lint test frontend-check check docker-validate docker-up docker-down docker-down-volumes docker-logs docker-status up down

init:
	@echo "Installing git hooks (pre-commit) and running initial checks"
	pre-commit install || echo "pre-commit not installed (pip install pre-commit)"
	cargo fmt --all
	cargo clippy --all-targets --all-features -- -D warnings
	cargo check

run:
	cargo run

# --- Development Workflow Targets ---

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

# Execute the smoke test against a running stack.
smoke:
	./scripts/smoke.sh

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	./scripts/test.sh

frontend-check:
	cd rib-react && npm run lint && npm test -- --run && npm run build

check: fmt lint test frontend-check

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

up:
	docker compose up -d

down:
	docker compose down
