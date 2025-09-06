## RIB Development Workflow & Environment Design

### Purpose
Define a fast inner development loop (debuggable Rust + React locally) while preserving confidence that the system works in its containerized production form. This document captures the current codebase realities and a concrete implementation checklist for tooling/parity improvements.

### Current State (Codebase Reference)
| Concern | Current Implementation (as of commit `main`) |
|---------|----------------------------------------------|
| Backend framework | Actix Web (`src/main.rs`, `routes.rs`) |
| Repo modes | Postgres backend (default) |
| Bind address | `0.0.0.0:8080` (recent change for container networking) |
| Auth | JWT (see `src/auth.rs`), Discord OAuth (requires env vars) |
| Image storage | Filesystem under `/app/data/images/<prefix>/<hash>` (see `upload_image` in `routes.rs`); NOT YET using MinIO bucket |
| Static / frontend | Built React app served by Nginx (see `rib-react/Dockerfile`, `rib-react/nginx.conf`) |
| Image fetch path | Backend route `GET /images/{hash}` (unscoped, added in `routes::config`) proxied by Nginx `location /images/` |
| Security headers | Custom middleware (`src/security.rs`) + Nginx headers |
| Data services | Postgres, Redis, MinIO via `docker-compose.yml` |
| Tests | Integration & route tests in `tests/` (run with default feature set) |
| Env config | `.env` (developer convenience) + explicit `environment:` section in Compose |

### Goals
1. Fast iteration (hot reload, debugger attachment) for both Rust backend and React frontend.
2. High confidence that production (container) images behave identically before merge.
3. Minimize “works locally / breaks in container” drift (dependencies, env vars, migrations, security headers, image handling).
4. Provide reproducible scripted smoke tests.
5. Prepare path to move image blobs from local FS → MinIO without blocking current work.

### Development Modes
| Mode | Description | Commands / Tooling |
|------|-------------|--------------------|
| Inner Loop (Local) | Local Rust + Vite dev server; infra via containers | `docker compose up -d postgres redis minio`; `cargo watch -x 'run'`; `npm run dev` |
| Parity Check (Container) | Build & run full images (Rust release build + Nginx bundle) | `docker compose build && docker compose up -d` |
| Feature Variant Check | Run tests with Postgres feature | `cargo test --no-default-features --features postgres-store` (optionally inside container) |
| Smoke Test | Hit critical endpoints & asset path | Script or Make target (see checklist) |

### Recommended Tooling Additions
1. **Makefile targets** (extend existing `Makefile`):
   - `make dev-infra` → `docker compose up -d postgres redis minio`
   - `make dev-backend` → run `cargo watch -x 'run'`
   - `make dev-frontend` → run `npm run dev --prefix rib-react`
   - `make smoke` → execute a shell script issuing curl calls (boards list, image fetch, docs, health checks).
   - `make build-images` → `docker compose build`.
2. **docker-compose.override.yml** (dev-only):
   - Mount backend source: `./src:/app/src` (if you want in-container hot reload alternative).
   - Override backend command: `cargo watch -x 'run'`.
3. **Pre-push Git hook** (`.githooks/pre-push`): run `make build-images && make smoke` (optional opt-in) to catch container regressions.
4. **CI**: single backend (Postgres) simplifies matrix.
5. **Rust `dotenvy` (dev only)** to load `.env` automatically when running locally (production still passes env explicitly).
6. **MinIO integration (future)**: Replace filesystem writes in `upload_image` with S3 PutObject; configurable via feature flag `s3-image-store`.

### Drift Risks & Mitigations
| Risk | Mitigation |
|------|------------|
| Env var added to `.env` but missing in Compose | Add checklist gate: update Compose with every new required var (PR template item). |
| Filesystem-only dependencies (e.g., `file` command absence) | Build-time validation or install minimal tools if needed; prefer pure Rust detection libs (already using `infer`). |
| Route added but not proxied (e.g., images bug) | Smoke test includes critical path for images & API. |
| Feature-specific code untested (postgres-store) | CI matrix + local scripted run. |
| Browser caching stale HTML for binary routes | Added dedicated Nginx `location /images/`; optionally add `Cache-Control: no-cache` to index.html only. |

### Smoke Test Script (Concept)
```
#!/usr/bin/env bash
set -euo pipefail
echo "[smoke] boards"; curl -fsS http://localhost:8080/api/v1/boards > /dev/null
echo "[smoke] docs";   curl -fsS http://localhost:8080/docs/openapi.json > /dev/null
echo "[smoke] image route (expect 404 OK)"; curl -Is http://localhost:8080/images/doesnotexist | grep -q '404'
echo "[smoke] frontend index"; curl -fsS http://localhost:3000/ > /dev/null || true
echo "[smoke] nginx image proxy 404"; curl -Is http://localhost:3000/images/doesnotexist | grep -q '404'
echo "[smoke] PASS"
```

### Migration Toward MinIO for Images (Future Design Excerpt)
1. Introduce feature flag `s3-image-store` in `Cargo.toml`.
2. Abstract storage behind a trait (e.g., `ImageStore` with `put(bytes) -> {hash, mime}`, `get(hash) -> Option<(bytes, mime)>`).
3. Provide `S3ImageStore` (MinIO via `aws-sdk-s3`). Filesystem storage has been removed.
4. Inject via `AppState` at startup based on feature + env (`S3_ENDPOINT`, credentials). 
5. Update tests to parametrize or add a small in-memory stub for unit tests.

### Quick Reference Commands
| Task | Command |
|------|---------|
| Start infra only | `docker compose up -d postgres redis minio` |
| Run backend locally | `cargo watch -x 'run'` |
| Run backend with Postgres feature | `cargo watch -x 'run --no-default-features --features postgres-store'` |
| Run frontend dev | `(cd rib-react && npm run dev)` |
| Container parity test | `docker compose build && docker compose up -d` |
| Smoke test (after script) | `make smoke` |

### Decision Log (Chronological Key Points)
| Date (UTC) | Decision | Rationale |
|------------|----------|-----------|
| 2025-09-04 | Bind backend to 0.0.0.0 | Enable Nginx container access (fixed 502) |
| 2025-09-04 | Add `/images/` Nginx proxy | Prevent React index fallback for binary objects |
| 2025-09-04 | Graceful 503 for missing Discord OAuth | Clearer UX vs 500 internal error |

### Open Questions
1. Do we want image writes to be eventually consistent (async task) or sync (current)?
2. Should we sign image URLs (when moving to MinIO) or keep them public hashed objects?
3. Add rate limiting / abuse protection (uploads)? (Future middleware / reverse proxy adjustments.)

### Summary
This workflow separates the “fast edit” path from the “confidence” path while using a lightweight checklist to keep environments in sync. Implementing the checklist incrementally will reduce integration friction and prepare the codebase for a future S3 / MinIO image backend.
