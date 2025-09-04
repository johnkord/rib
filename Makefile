init:
	@echo "Installing git hooks (pre-commit) and running initial checks"
	pre-commit install || echo "pre-commit not installed (pip install pre-commit)"
	cargo fmt --all
	cargo clippy --all-targets --all-features -- -D warnings || true
	cargo check

run:
	cargo run --features inmem-store

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
	@echo "Starting dependencies for local development (postgres, redis, minio)..."
	docker compose up -d postgres redis minio

up:
	docker compose up -d

down:
	docker compose down -v
