# Implementation Checklist – RIB

> Tick items off as they are merged into `main`.

## 0. Repo & Tooling
- [x] Initialise git hooks (`pre-commit`, `cargo fmt`, `cargo clippy`)
  - [x] Add `.pre-commit-config.yaml`
  - [x] `pre-commit install` in `Makefile init`
  - [ ] CI step `pre-commit run --all-files --show-diff-on-failure` (optional future – current CI runs fmt/clippy directly)
- [x] Set up CI jobs (lint, test, audit, build, docker)
  - [x] GitHub Actions matrix (stable/beta/nightly)
  - [x] Cache cargo registry & `target` dir
  - [x] Job `cargo-audit` + SARIF upload
  - [x] Push Docker image to GHCR on tag
- [x] Configure dev-container for VS Code
  - [x] `devcontainer.json` with mounts for cargo cache
  - [x] Extensions: rust-analyzer, CodeLLDB
  - [x] Post-create command runs `cargo check`
- [x] Add `.env.example` and sample `docker-compose.yml`
  - [x] Services: postgres, redis, minio, mailhog
  - [x] Document `make up / make down` helpers

## 1. Core Models & Storage (v0.1)
- [x] Define Rust structs
  - [x] `Board { id, slug, title, created_at }`
  - [x] `Thread { id, board_id, subject, bump_time }`
  - [x] `Reply { id, thread_id, content, created_at }`
  - [x] `Image { id, thread_id?, reply_id?, hash, mime }`
  - [x] `Report { id, target_id, reason, created_at }`
- [x] Implement `sqlx` migrations
  - [x] Create tables with FK + ON DELETE CASCADE
  - [x] Indices: `(board_id, bump_time DESC)` on `threads`
  - [x] Seed migration for sample board `general`
- [x] Add in-memory repository behind `dyn` trait
  - [x] Generic trait `Repo<T>` (simplified unified Repo trait)
  - [x] Feature flag `inmem-store`
- [x] Map models to JSON (serde) & database (sqlx)
  - [x] Derive `Serialize`, `Deserialize`
  - [x] Implement `FromRow` for each (feature-gated `postgres-store`)

## 2. REST API (v0.1)
- [x] Set up Actix-web skeleton
  - [x] Middleware: logger, compression
  - [x] App state holds repo trait object
- [x] Versioned base path `/api/v1`
- [x] CRUD endpoints  
  - [x] Boards  
        - [x] `GET /boards`  
        - [x] `POST /boards`
  - [x] Threads  
        - [x] `GET /boards/{id}/threads`  
        - [x] `POST /threads`
  - [x] Replies  
        - [x] `GET /threads/{id}/replies`  
        - [x] `POST /replies`
- [x] OpenAPI generation (`utoipa`)
  - [x] Auto-generate JSON at build, serve `/docs/openapi.json`
  - [x] Swagger-UI at `/docs`
- [x] Error handling middleware
  - [ ] Convert `anyhow::Error` → `ApiError` (not yet needed; using RepoError)
  - [x] Map to HTTP status & JSON body

## 3. Images (v0.1 → v0.5)
- [x] Streaming upload endpoint (`POST /images`)
  - [x] Limit: 10 MB, timeout 30 s (timeout TBD; size enforced)
- [x] Enforce size & MIME limits early
  - [x] Reject non png/jpeg/gif/webp
- [x] SHA-256 deduplication
  - [x] Return 409 on duplicate hash
- [x] Local FS storage (dev) behind simple logic (trait pending)
  - [x] Path: `./data/images/{hash[0..2]}/{hash}`
- [ ] Unit tests for image validation
 - [x] Unit tests for image validation
  - [x] Corpus of valid & invalid samples (basic set in tests)

## 3½. Frontend (rib-web)
- [x] Project scaffold (`npm create svelte@latest rib-web`)
  - [x] TypeScript, ESLint, Prettier
  - [x] Tailwind CSS setup
- [x] Routing
  - [x] `/` boards list
  - [x] `/b/{slug}` board catalog
  - [x] `/thread/{id}` thread view
- [ ] Forms
 - [x] New thread form with image upload (basic; image hash not yet linked to thread model)
 - [x] Reply form (inline basic version; modal enhancement pending)
- [x] API client
  - [x] `fetchJson` wrapper with error handling
  - [x] React Query / TanStack Query integration
- [ ] Auth
 - [ ] JWT login modal (moderator) (stub modal & store added)
 - [ ] Role-based route guard (pending backend roles)
- [ ] CI
 - [x] GitHub Actions: lint, build (tests none yet)
  - [ ] Deploy preview via Netlify/Vercel
- [ ] Docker
 - [x] Multi-stage build producing static files
 - [x] Nginx container serving `/` with SPA fallback
- [ ] Accessibility & a11y audit script
- [ ] Lighthouse performance budget in CI

## 4. Security Baseline
- [ ] JWT middleware with role claims
  - [ ] HS256 secret from `JWT_SECRET`
  - [ ] Roles: user, moderator, admin
- [ ] Rate-limiting (in-memory fallback)
  - [ ] Sliding window in `dashmap`
- [ ] CSRF token check for mutating verbs
  - [ ] Double-submit cookie strategy
- [ ] Security headers middleware
  - [x] Security headers middleware
  - [x] CSP / HSTS / Referrer-Policy presets (HSTS via ENABLE_HSTS env var)

## 5. Caching & Performance (v0.5)
- [ ] Integrate Redis pool (feature-flagged)
  - [ ] Deadpool-redis config from `REDIS_URL`
- [ ] Token bucket rate-limit Lua script
  - [ ] Script stored in `resources/lua`
- [ ] Catalog cache (`boards/{id}/threads`)
  - [ ] Cache stampede lock with `SETNX`
- [ ] Compress responses (brotli/gzip)

## 6. External Storage & Services (v0.5)
- [ ] PostgreSQL repository
  - [ ] Feature `postgres-store`
  - [ ] Connection pool with `deadpool-postgres`
- [ ] S3 image store adapter
  - [ ] MinIO config for local dev
- [ ] Background thumbnail job
  - [ ] Worker binary `rib-thumbd`
  - [ ] Queue: Redis list `thumbq`

## 7. Moderation & Reports (v1.0)
- [ ] CRUD endpoints for `Report`
  - [ ] `POST /reports` (public)
  - [ ] `GET /reports` (moderator)
- [ ] Role-based access checks
- [ ] Audit log table + middleware
  - [ ] Insert on every DELETE/PATCH
- [ ] CAPTCHA integration
  - [ ] Turnstile server-side verify endpoint

## 8. Search & Text Indexes (v1.0)
- [ ] PostgreSQL FTS migration
  - [ ] `to_tsvector('simple', content)`
- [ ] `/search` endpoint
  - [ ] Filters: board, author, date
- [ ] Ranking & pagination tests

## 9. WebSocket Live Updates (v1.0)
- [ ] WS endpoint `/ws/thread/{id}`
- [ ] Broadcast on new reply via channel
- [ ] Client heartbeat & disconnect

## 10. Observability
- [ ] `tracing` + OTEL exporter
  - [ ] OTLP endpoint configurable
- [ ] Prometheus metrics
  - [ ] `actix-web-prom` middleware
- [ ] Health & readiness probes
- [ ] k6 load-test scripts in `tests/perf`

## 11. CI/CD Enhancements
- [ ] Blue-green deployment workflow
- [ ] Smoke tests post-deploy
- [ ] Auto-tag release on `CHANGELOG.md` update

## 12. Documentation
- [ ] Update OpenAPI file on build
- [ ] Add runbooks for SEV-1/2
- [ ] Publish Docker image usage guide

## 13. Scale & Future (v2.0)
- [ ] Extract auth service (POC)
- [ ] GraphQL gateway (`async-graphql`)
- [ ] ActivityPub federation prototype
