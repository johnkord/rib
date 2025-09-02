# Implementation Checklist – RIB

> Tick items off as they are merged into `main`.

## 1. Frontend (rib-react)
- [x] React 18 setup with TypeScript
  - [x] Vite build tooling
  - [x] Tailwind CSS + DaisyUI
- [x] Core routing with React Router v6
  - [x] BoardsPage (`/`)
  - [x] BoardThreadsPage (`/b/:slug`)
  - [x] ThreadPage (`/thread/:id`)
  - [x] LoginPage (`/login`)
  - [x] AdminRoles (`/admin/roles`)
- [x] Authentication
  - [x] Discord OAuth flow integration
  - [x] JWT storage in localStorage
  - [x] useAuth hook for user state
  - [x] Token capture from URL params
- [x] Data fetching with @tanstack/react-query
  - [x] Board CRUD hooks (useBoards, useCreateBoard, useUpdateBoard)
  - [x] Thread hooks (useThreads, useCreateThread)
  - [x] Reply hooks (useReplies, useCreateReply)
- [x] Media handling
  - [x] Image upload with multipart/form-data
  - [x] Video support (mp4, webm)
  - [x] MediaModal viewer with navigation
  - [x] Thumbnail preview in thread lists
- [x] User features
  - [x] Reply highlighting via URL hash
  - [x] Smooth scroll to reply
  - [x] Board editing inline
  - [x] Error handling with user feedback
- [ ] Advanced features
  - [ ] Markdown support in posts
  - [ ] Quote linking (>>123)
  - [ ] Search functionality
  - [ ] WebSocket live updates
  - [ ] Infinite scroll for long threads
  - [ ] Drag-and-drop file upload

## 4. Security Baseline
- [x] JWT middleware with role claims
  - [x] HS256 secret from `JWT_SECRET`
  - [x] Roles: user, moderator, admin
- [ ] Rate-limiting (in-memory fallback)
  - [ ] Sliding window in `dashmap`
- [ ] CSRF token check for mutating verbs
  - [ ] Double-submit cookie strategy
- [ ] Security headers middleware
  - [ ] CSP / HSTS / Referrer-Policy presets (HSTS via ENABLE_HSTS env var)

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
- [ ] ActivityPub federation prototype
