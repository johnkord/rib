#!/usr/bin/env bash
set -euo pipefail

if ! command -v sqlx >/dev/null 2>&1; then
  echo "sqlx CLI is required. Install it with:" >&2
  echo "  cargo install sqlx-cli --version 0.8.6 --no-default-features --features rustls,postgres --locked" >&2
  exit 1
fi

docker compose up -d --wait postgres

test_database="rib_test_$(date +%s)_$$"
cleanup() {
  docker compose exec -T postgres dropdb -U postgres --if-exists "$test_database" >/dev/null
}
trap cleanup EXIT INT TERM

docker compose exec -T postgres createdb -U postgres "$test_database"

export DATABASE_URL="postgres://postgres:postgres@127.0.0.1:5432/$test_database"
export JWT_SECRET="${JWT_SECRET:-test-jwt-secret-abcdefghijklmnopqrstuvwxyz012345}"
export TRIPCODE_SECRET="${TRIPCODE_SECRET:-test-tripcode-secret-abcdefghijklmnopqrstuvwxyz}"

sqlx migrate run
cargo test --all-features --no-fail-fast -- --test-threads=1 "$@"
