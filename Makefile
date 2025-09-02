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

up:
	docker compose up -d

down:
	docker compose down -v
