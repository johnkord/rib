# RIB - Rust Image Board

> Status: Early-stage (pre-1.0). Interfaces may change. Feedback & contributions welcome.

## 1. Overview

### 1.1 Purpose
RIB (Rust Image Board) is a high-performance, self-hostable image board backend built in Rust. It provides a RESTful API for managing posts, images, comments, and user interactions in a thread-based discussion format similar to traditional image boards.

### 1.2 Goals
- Fast & resource-light
- Horizontally scalable
- Pluggable storage/auth
- Memory-safe (Rust)
- Simple ops

### 1.3 Non-Goals
- Real-time (post-v1)

### 1.4 Glossary
Thread - top-level post  
Reply - comment in a thread  
Board - category of threads  
Bump - reply that lifts a thread  
Sage - non-bumping reply

## 2. Architecture
Single Actix-web service backed by:
• pluggable RDBMS (PostgreSQL, SQLite)  
• optional Redis cache  
• S3-compatible object store for file attachments

Key layers: API → Service → Repository → Storage/Cache.

## 3. Data Models
See `src/models.rs` for full Rust definitions:
Board, Thread, Reply, File, Report (+ enums).

Validation highlights:  
• board.slug: `^[a-z0-9_]{1,12}$`  
• post.content: 1-2000 chars  
• attachments ≤ 25 MB, supports images, videos, documents, archives, and other common file types

## 4. API
OpenAPI spec: `/docs/api/openapi.yaml`  
Major groups: boards, threads, replies, files, moderation.  
Supports idempotency (header `Idempotency-Key`) and versioned base path `/api/v1`.

### 4.1 File Upload Support
The system supports uploading various file types via the `/api/v1/images` endpoint:

**Supported File Types:**
- **Images**: PNG, JPEG, GIF, WebP, BMP, TIFF, SVG
- **Videos**: MP4, WebM, AVI, MOV, WMV, FLV  
- **Audio**: MP3, WAV, OGG, FLAC, AAC, M4A
- **Documents**: PDF, Word (DOC/DOCX), Excel (XLS/XLSX), PowerPoint (PPT/PPTX), RTF, OpenDocument formats
- **Text/Code**: Plain text, CSV, HTML, CSS, JavaScript, JSON, XML, YAML
- **Archives**: ZIP, RAR, 7-Zip, TAR, GZIP, BZIP2
- **Generic binary files**: application/octet-stream

**Limits:**
- Maximum file size: 25 MB
- Content-Type detection via magic bytes
- SHA256 deduplication prevents duplicate storage

## 5. Storage Strategy

### 5.1 Database Schema
Primary storage uses PostgreSQL with the following considerations:
- UUID primary keys for global uniqueness
- Composite indexes on (board_id, bump_time) for thread listing
- Full-text search indexes on content fields
- Partitioning by board for large deployments

### 5.2 Caching Strategy
- **Redis** for hot data:
  - Board catalog (5-minute TTL)
  - Thread previews (1-minute TTL)
  - Rate limiting counters
  - Session data

### 5.3 File Storage
- **Primary**: S3-compatible object storage (AWS S3, MinIO, etc.)
- **CDN**: CloudFlare or similar for global distribution
- **Thumbnails**: Generated on upload for images, stored separately
- **Deduplication**: SHA256 hash checking before storage

### 5.4 Backup & Disaster Recovery
- **Database**: Point-in-time recovery (WAL shipping to cold storage).  
- **Files**: Cross-region replication in object storage.  
- **Config**: Encrypted off-site backups of `.env` and Kubernetes secrets.  
- Quarterly recovery drills verify backup integrity.

### 5.5 Data Retention & Lifecycle
| Data | Retention | Action |
|------|-----------|--------|
| Threads (active) | indefinite | None |
| Threads (archived) | 365 days default | Purge files, keep metadata |
| Reports | 180 days | Hard delete |
| Audit logs | 730 days | Glacier/offline archive |
| Hashed IPs | 90 days | Rotating re-hash + purge old salts |

### 5.6 File Processing Pipeline
1. Upload received (streamed, max size enforced early).
2. MIME sniff + magic number verify.
3. Hash (SHA256) while streaming -> dedupe check.
4. Temporary quarantine storage.
5. Optional scanning (clamd / external API).
6. For images: Resize + thumbnail (libvips recommended).
7. Commit metadata row + move to final bucket path.
8. Emit event (internal) for cache warm.

### 5.7 Migration Strategy
- Use `sqlx migrate` with strictly forward-only migrations in main branch.
- Emergency rollback: deploy prior binary + run compensating forward migration (no down scripts in prod).
- Preflight: CI runs `sqlx prepare` to verify offline query metadata.

## 6. Security Considerations

### 6.1 Authentication & Authorization
- **Anonymous posting**: Default, with optional tripcodes
- **Moderator auth**: JWT tokens with role-based permissions
- **API keys**: For automated tools and bots

### 6.2 Rate Limiting
- Per-IP write limits:
  - 1 thread per 5 minutes
  - 10 replies per minute
  - 5 file uploads per hour
- **Per-IP read cap** (ingress/CDN): ~120 requests per minute (burst 240)  
- Exponential backoff for repeated violations

### 6.3 Content Security
- **CAPTCHA**: Required for thread creation
- **Spam detection**: Bayesian filter for common spam patterns
- **File validation**: File type verification, virus scanning
- **XSS prevention**: Sanitize all user input
- **CSRF protection**: Token validation for state-changing operations

### 6.4 Privacy
- **IP logging**: Hashed IPs only, rotated monthly
- **GDPR compliance**: Data export and deletion APIs
- **No tracking**: No analytics or third-party scripts

### 6.5 Compliance
- **DMCA / EUCD**: Takedown workflow via `/api/reports`.  
- **COPPA**: 13+ age gate banner for US visitors.  
- **Accessibility**: WCAG 2.1 AA targets for future frontend.

### 6.6 Offensive-Content Detection
Pluggable hook that sends file hashes to third-party services
(e.g., PhotoDNA) before the file is made public. Failing files are quarantined.

### 6.7 Security Headers & Hardening
Returned default headers:
- `Content-Security-Policy: default-src 'none'; img-src 'self' data: https:;`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: no-referrer`
- `Permissions-Policy: interest-cohort=()`
- `Strict-Transport-Security: max-age=63072000`
TLS: Recommend TLS 1.3 only (configurable fallback).

### 6.8 Vulnerability Management
- Weekly `cargo audit` in CI.
- Monthly dependency freshness report.
- CVE triage SLA: Critical 24h, High 72h, Medium 14d.

### 6.9 Edge Protection & WAF (Optional Production Layer)
> This section was consolidated from the former `docs/design.md`.

RIB assumes basic application‑layer limits; for internet exposure you should pair it with an edge / CDN / WAF service to shed volumetric or brute‑force traffic earlier.

Typical pattern (example: Azure Front Door + WAF, but Cloudflare / Fastly / AWS equivalents work):
* Global rate limit (broad per‑IP cap, e.g. 100 req/min) before origin
* Narrow auth brute force rule (e.g. 5 login attempts/min on `/api/auth/login`)
* (Optional) geo, bot, or reputation based rules

Minimal rule table:
| Rule | Threshold | Window | Scope | Purpose |
|------|-----------|--------|-------|---------|
| GlobalRateLimit | 100 | 1 min | All paths | Baseline volumetric damping |
| AuthRateLimit | 5 | 1 min | `/api/auth/login` | Credential stuffing mitigation |

Operational notes:
* Tune after observing real traffic (export WAF logs to metrics store).
* Keep application aware only of business action limits (threads/replies/images) to reduce complexity.
* Consider synthetic probes for health endpoints so WAF doesn’t learn poor baselines.

Enhancements (future): adaptive thresholds, WAF blocked‑request alerting, automated CAPTCHA challenge escalation after repeated violations.

### 6.10 Soft & Hard Deletion (Moderation Lifecycle)
Implements reversible soft deletion and irreversible hard deletion for Boards, Threads, and Replies.

Goals:
* Soft delete hides content (`deleted_at` timestamp) but allows restoration.
* Hard delete permanently removes (leveraging FK cascades).
* Non‑admins always receive 404 for soft‑deleted content (no enumeration leaks).
* Admins can opt‑in to view deleted entities with `?include_deleted=1`.

Schema additions: nullable `deleted_at TIMESTAMPTZ` on `boards`, `threads`, `replies` plus partial indexes on active rows:
```
CREATE INDEX idx_boards_not_deleted ON boards(id) WHERE deleted_at IS NULL;
CREATE INDEX idx_threads_board_active ON threads(board_id, bump_time DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_replies_thread_active ON replies(thread_id, created_at ASC) WHERE deleted_at IS NULL;
```

API (admin only):
```
POST   /api/v1/admin/boards/{id}/soft-delete
POST   /api/v1/admin/boards/{id}/restore
DELETE /api/v1/admin/boards/{id}
POST   /api/v1/admin/threads/{id}/soft-delete
POST   /api/v1/admin/threads/{id}/restore
DELETE /api/v1/admin/threads/{id}
POST   /api/v1/admin/replies/{id}/soft-delete
POST   /api/v1/admin/replies/{id}/restore
DELETE /api/v1/admin/replies/{id}
```

Query parameter: `include_deleted=1` (honored only for admins) on selected GETs:
* `GET /api/v1/boards`
* `GET /api/v1/boards/{id}/threads`
* `GET /api/v1/threads/{id}`
* `GET /api/v1/threads/{id}/replies`

Visibility rules:
| Actor | Deleted entity (no include) | With `include_deleted=1` |
|-------|-----------------------------|--------------------------|
| Non‑admin | 404 | 404 |
| Admin | 404 | 200 + `deleted_at` |

Repository patterns:
* Soft delete: `UPDATE <table> SET deleted_at = COALESCE(deleted_at, now()) WHERE id=$1`.
* Restore: `UPDATE <table> SET deleted_at = NULL WHERE id=$1`.
* Hard delete: `DELETE FROM <table> WHERE id=$1`.
* Listings filter on `deleted_at IS NULL` unless admin + include flag.

Edge cases tested:
1. Soft delete hides from non‑admin lists/detail.
2. Admin include flag reveals with `deleted_at` populated.
3. Restore returns entity to active listings.
4. Soft‑deleted thread hides replies (thread 404 for non‑admin).
5. Hard delete cascades replies.
6. Idempotent soft delete / restore.

Future enhancements (not yet implemented): deletion reasons, `deleted_by`, moderation audit log, scheduled purge, bulk moderation actions.

> Design rationale consolidated here; original standalone design document removed to prevent drift.

## 7. Performance Optimizations

### 7.1 Database
- Connection pooling with `deadpool-postgres`
- Prepared statements for common queries
- Read replicas for scaling reads
- Materialized views for catalog pages

### 7.2 Application
- Async I/O throughout with Tokio
- Response compression (gzip/brotli)
- ETag headers for caching
- Lazy loading of images

### 7.3 Infrastructure
- Horizontal scaling behind load balancer
- Auto-scaling based on CPU/memory metrics
- Geographic distribution with edge caching

### 7.4 Capacity Planning (Initial Targets)
| Metric | Target |
|--------|--------|
| P95 thread list latency | < 80 ms |
| P99 reply post latency | < 150 ms |
| Max sustained RPS (single node) | 800 (read-heavy mix) |
| Cache hit ratio (catalog) | > 85% |
Load tests executed prior to tagged releases; thresholds enforced in CI performance stage.

### 7.5 Rate Limiting Implementation

Current (MVP) implementation: in-memory sliding window (per-process) using a lock-free `dashmap`.

Scopes & defaults (override with env vars):
- Threads: 1 per 300s (`RL_THREAD_LIMIT=1`, `RL_THREAD_WINDOW=300`)
- Replies: 10 per 60s (`RL_REPLY_LIMIT=10`, `RL_REPLY_WINDOW=60`)
- Images: 5 per 3600s (`RL_IMAGE_LIMIT=5`, `RL_IMAGE_WINDOW=3600`)

Activation: set `RL_ENABLED=1` (or `true`). If disabled, limiter is bypassed.

Algorithm: sliding window of `Instant`s pruned on each check (O(k) with k << n based on small window sizes). Chosen for simplicity and low per-request overhead. Metrics exposed:
- `rate_limit_allowed{action}`
- `rate_limit_denied{action}`

Limitations:
- Not distributed: each pod enforces independently (acceptable while upstream gateway/WAF supplies coarse global caps).

#### 7.5.1 Future: Distributed Token Bucket (Redis)
The original design doc also outlined a Redis-backed token bucket for strict global coordination. It was deferred for simplicity but remains the upgrade path when:
* > ~10 pods and per‑pod multiplication becomes abusable
* Need for precise global quotas / billing metrics
* Desire for shared ban lists or reusable buckets across actions

Sketch (atomic Lua script pattern):
```
KEY: rl:{scope}:{normalized_ip}
Fields: tokens (int), refreshed_at (unix seconds)
Inputs: capacity, refill_rate (tokens/sec)
Algorithm:
  now = time()
  elapsed = now - refreshed_at
  tokens = min(capacity, tokens + elapsed * refill_rate)
  if tokens >= 1 then tokens -= 1; allow else deny
```

Pros:
* Single global view; exact fairness
* Easy temporary bans (expire key or set 0 tokens)

Cons:
* Adds infrastructure (Redis) for something edge/WAF already mitigates at coarse granularity
* Higher latency (extra network hop) vs in‑process

Migration approach:
1. Ship feature-flagged Redis limiter alongside in-memory (shadow mode metrics)
2. Compare allowance/denial divergence
3. Flip default when divergence < acceptable threshold
4. Remove in-memory path or keep as fallback

Until then the in‑process sliding window plus edge layer is considered "good enough" for early adoption.

## 8. Monitoring & Observability

### 8.1 Metrics
- **Application metrics**: Request rate, latency, error rate
- **Business metrics**: Posts per hour, active users
- **Infrastructure metrics**: CPU, memory, disk I/O

### 8.2 Logging
- Structured logging with `tracing`
- Log levels: ERROR, WARN, INFO, DEBUG
- Centralized aggregation with ELK or similar

### 8.3 Tracing
- Distributed tracing with OpenTelemetry
- Request ID propagation
- Performance profiling endpoints

### 8.4 Alerting & Incident Response
- PagerDuty integration for P90 latency > 1 s or error rate > 2 %. 
- Post-mortems required within 48 h of SEV-1 incidents.

### 8.5 Metrics Naming (Prometheus)
- HTTP: `http_requests_total{method,route,status}`
- Latency: `http_request_duration_seconds_bucket{route}`
- DB: `db_query_duration_seconds_bucket{query_type}`
- Cache: `cache_operations_total{op,result}`
- Domain: `threads_created_total`, `replies_created_total`, `reports_open_total`

### 8.6 Red/USE Dashboards
- RED: Rate, Errors, Duration per top 10 routes.
- USE: Utilization, Saturation, Errors per resource (CPU, worker queue, DB pool).

## 9. Development Workflow

### 9.1 Local Development
```bash
# Start dependencies
docker-compose up -d postgres redis minio

# Run migrations
sqlx migrate run

# Start dev server with hot reload
cargo watch -x run
```

### 9.2 Testing Strategy
- **Unit tests**: Business logic validation
- **Integration tests**: API endpoint testing
- **Load tests**: Performance benchmarking with k6
- **Security tests**: OWASP ZAP scanning

### 9.3 CI/CD Pipeline
1. Lint with `clippy` and `rustfmt`
2. Run test suite
3. Security audit with `cargo-audit`
4. Build Docker image
5. Deploy to staging
6. Run smoke tests
7. Deploy to production (blue-green)

### 9.4 Tooling
- **Pre-commit**: `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`
- **Dev-container**: `.devcontainer` folder for VS Code + Docker ensuring reproducible envs.

### 9.5 Local Profiles
- `RUST_LOG=debug RIB_PROFILE=dev` enables verbose SQL + feature flags.
- `RIB_FEATURES="unsafe-fast-hash"` (if added) would aid rapid prototyping (never in prod).

### 9.6 Database Seeding
`cargo run --bin seed` populates sample boards & threads (feature-gated).

### 9.7 Code Generation
Planned: OpenAPI spec -> typed client via `oapi-codegen` (future).

## 10. Deployment

### 10.1 Docker Configuration
```dockerfile
FROM rust:1.75 as builder
# Multi-stage build for minimal image size

FROM debian:bookworm-slim
# Runtime with only necessary dependencies
```

### 10.2 Kubernetes Deployment
- **Deployment**: 3+ replicas for HA
- **Service**: LoadBalancer or NodePort
- **ConfigMap**: Environment configuration
- **Secret**: Database credentials, API keys
- **HPA**: Auto-scaling based on metrics

### 10.3 Environment Variables
```env
DATABASE_URL=postgres://user:pass@host/db
REDIS_URL=redis://host:6379
S3_BUCKET=rib-images
S3_ENDPOINT=https://s3.amazonaws.com
JWT_SECRET=<secure-random>
RUST_LOG=info
```

### 10.4 Configuration Management
Priority order:
1. Environment variables
2. `config/{env}.toml`
3. CLI flags (override all)
Validation at startup; process aborts on missing required keys.

### 10.5 Zero-Downtime Rolling Update
- Use readiness probe (healthz + version).
- Stagger termination grace period > max request time (e.g., 30s).
- Migrations run before pod replacement for additive changes.

## 11. Roadmap

### Phase 1: MVP (v0.1)
- [x] Basic CRUD for threads and replies
- [ ] In-memory storage for development
- [ ] Simple image upload
- [ ] Basic catalog view

### Phase 2: Production Ready (v0.5)
- [ ] PostgreSQL integration
- [ ] Redis caching
- [ ] S3 image storage
- [ ] Rate limiting
- [ ] CAPTCHA integration

### Phase 3: Advanced Features (v1.0)
- [ ] Full moderation tools
- [ ] Search functionality
- [ ] WebSocket for live updates
- [ ] Archive system
- [ ] API versioning

### Phase 4: Scale (v2.0)
- [ ] Microservices architecture
- [ ] GraphQL API option
- [ ] Federation support
- [ ] Machine learning for spam detection

### Risk Register (New)
| Risk | Mitigation |
|------|------------|
| Image hash collision | SHA256 adequate; also check size/dimensions |
| Cache stampede on hot thread | Use request coalescing mutex (Redis SETNX) |
| Moderator key leak | Short-lived JWT + rotation schedule |
| Slow external hash check | Async queue + temporary placeholder thumbnail |

## 12. Contributing

### 12.1 Code Style
- Follow Rust standard conventions
- Use `rustfmt` for formatting
- Write descriptive commit messages
- Add tests for new features

### 12.2 Pull Request Process
1. Fork the repository
2. Create feature branch
3. Write tests and documentation
4. Submit PR with description
5. Address review feedback

### 12.3 Coding Guidelines (Additions)
- Prefer ` anyhow::Result` internally; map to API errors at handler boundary.
- Use `#[instrument(skip(body))]` for handlers; avoid logging raw images.

## 13. License
MIT License - See LICENSE file for details

## 14. Future Extensions (Exploratory)
- Pluggable ML scoring for spam (ONNX runtime).
- Adaptive image quality tiering (WebP/AVIF) negotiated via `Accept` header.
- Federation via ActivityPub subset for cross-instance thread mirroring.

## 15. Open Questions
| Topic | Question |
|-------|----------|
| Search | Use Postgres FTS vs external (Meilisearch) for scale > 10M posts? |
| Archive | Cold storage compression strategy? |
| Abuse | Automatic rate adaptation under volumetric attack? |
| Internationalization | How to store per-thread locale metadata? |

## 16. Decision Log (Initial)
| ID | Decision | Rationale | Status |
|----|----------|-----------|--------|
| D1 | PostgreSQL primary store | Reliability + indexing | Accepted |
| D2 | Actix-web framework | Performance + ecosystem | Accepted |
| D3 | Redis optional cache | Avoid hard dependency for MVP | Accepted |
| D4 | S3 for images | Scalability vs local FS | Accepted |


## 17. Browser Frontend (rib-web)

### 17.1 Overview  
A single-page application (SPA) that consumes the `/api/v1` REST endpoints provided by RIB. It lives in a sibling repository `rib-web`, but its requirements are documented here to keep the product design cohesive.

### 17.2 Tech Stack  
• Framework: SvelteKit (lightweight, great SSR support)  
• Language: TypeScript  
• Styling: Tailwind CSS + DaisyUI components  
• State: Svelte stores (board/thread cache)  
• HTTP: `@tanstack/query` for request caching & deduplication  
• Auth: JWT (stored in `HttpOnly` cookie) - shares secret with API  
• Build: Vite (bundles, hot reload), ESLint + Prettier

### 17.3 Core Pages  
| Route | Description |
|-------|-------------|
| `/` | Global board list |
| `/{slug}` | Thread catalog for a board |
| `/thread/{id}` | Thread view with infinite scroll replies |
| `/thread/new` | Create thread (CAPTCHA, image upload) |
| `/reply/{threadId}` | Quick-reply modal |
| `/mod` | Moderator dashboard (role-gated) |

### 17.4 Components  
- `ImageUpload` - drag-and-drop, progress bar, MIME/size pre-check  
- `PostForm` - markdown editor with live preview  
- `ThreadCard` - board catalog entry  
- `Reply` - collapsible with quote-link parsing  
- `AuthGuard` - protects moderator routes

### 17.5 API Integration  
All requests hit the same origin (served under `/frontend`) or a separate domain with CORS configured. `fetch` wrapper injects JWT, handles refresh, and maps HTTP errors to toast notifications.

### 17.6 Deployment  
`rib-web` is built into static assets (`dist/`) and either:  
1. Served by Nginx side-car container, or  
2. Embedded in the Actix binary behind `/` when the `embed-frontend` feature is enabled (uses `include_bytes!`).

### 17.7 Accessibility & i18n  
- WCAG 2.1 AA compliance baseline  
- Internationalization via `svelte-i18n`, default `en-US`

### 17.8 Roadmap  
Phase 1 (v0.1) - read-only pages  
Phase 2 (v0.5) - posting flows + image upload  
Phase 3 (v1.0) - moderation UI, live updates via WS

## Getting Started

### TL;DR Quick Start
```bash
git clone https://github.com/johnkord/rib
cd rib
cp .env.example .env
openssl rand -base64 48 | tr -d '\n' | sed -e "s|CHANGE_ME_GENERATE_A_SECURE_SECRET|$(cat -)|" -i .env || true
docker compose up -d postgres redis minio
cargo run
# Open http://localhost:8080/docs
```

Or fully containerized (backend too):
```bash
docker compose up -d
```
Then visit:
 - API: http://localhost:8080
 - Docs: http://localhost:8080/docs
 - MinIO Console: http://localhost:9001 (minioadmin / minioadmin)

### Before Public / Production (Out-of-Band Checklist)
| Task | Reason |
|------|--------|
| Replace placeholder security/contact emails | Provide real disclosure channels |
| Set strong `JWT_SECRET` (48+ bytes) | Prevent JWT forgery |
| Harden Postgres creds & network policy | Reduce blast radius |
| Provision dedicated least-privilege S3 user | Principle of least privilege |
| Configure backups (DB WAL + object replication) | Disaster recovery |
| Enable TLS + set `ENABLE_HSTS=true` | Transport security |
| Add CI (fmt, clippy -D warnings, test, audit) | Supply chain & quality |
| External/global rate limiting (CDN/WAF) | Abuse mitigation |
| Review CSP before adding external scripts | Maintain XSS posture |

### Prerequisites
- Rust 1.75+
- Node.js 18+ (for frontend)
- Docker (optional, for dependencies)

### Setup

1. Clone the repository:
```bash
git clone https://github.com/yourusername/rib.git
cd rib
```

2. Configure environment variables  
  Copy `.env.example` to `.env` and adjust values.  

3. Configure Discord OAuth (optional):
   - Go to https://discord.com/developers/applications
   - Create a new application
   - Go to OAuth2 section
   - Add redirect URI: `http://localhost:8080/api/v1/auth/discord/callback`
   - Set `DISCORD_CLIENT_ID` and `DISCORD_CLIENT_SECRET` environment variables

4. Run the server:
```bash
cargo run
```

5. (Optional) Develop frontend separately:
The production build is now embedded in the Rust binary. For iterative frontend development you can still run Vite:
```bash
cd rib-react
npm install
npm run dev
```
When running the separate dev server, API calls target http://localhost:8080 and CORS is already configured.

To refresh the embedded assets used by the Rust binary during local (non-Docker) development:
```bash
cd rib-react
npm run build
cp -r dist ../embedded-frontend
cd ..
cargo run
```
The copy step updates the files that `rust-embed` packages at compile time; rebuild the Rust binary after changes.

### Development Workflow Reference
See `docs/dev-workflow.md` for the evolving development environment design, parity strategy, and implementation checklist.

Common convenience targets (frontend container removed; backend serves SPA):
```bash
make dev-infra      # start postgres, redis, minio
make dev-backend    # run backend with auto-reload (needs cargo-watch)
make dev-frontend   # run Vite dev server
make build-images   # build all Docker images
make smoke          # run quick curl-based smoke tests
```

### Git Hooks (Optional)
Install local git hooks (pre-push smoke + tests) by pointing git to the provided hooks directory:
```bash
git config core.hooksPath .githooks
```
Disable temporarily with:
```bash
git config --unset core.hooksPath
```

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `JWT_SECRET` | Yes | Secret key for JWT signing (min 32 chars; recommend 48) |
| `DISCORD_CLIENT_ID` | No | Discord OAuth application ID |
| `DISCORD_CLIENT_SECRET` | No | Discord OAuth application secret |
| `DISCORD_REDIRECT_URI` | No | OAuth callback URL |
| `FRONTEND_URL` | No | Public origin of the SPA (default: http://localhost:8080 when embedded) |
| `BOOTSTRAP_ADMIN_DISCORD_IDS` | No | Comma‑separated Discord user IDs granted Admin on first login (e.g. `188880431955968000`) |
| `ENABLE_HSTS` | No | Add HSTS header (only set true behind HTTPS) |
| `S3_ENDPOINT` / `S3_ACCESS_KEY` / `S3_SECRET_KEY` / `S3_BUCKET` | Yes (runtime) | Image storage (MinIO/S3) |
| `RL_ENABLED` | No | Enable in-memory rate limiting ("1"/"true") |
| `RL_THREAD_LIMIT` / `RL_THREAD_WINDOW` | No | Thread create limit & window seconds |
| `RL_REPLY_LIMIT` / `RL_REPLY_WINDOW` | No | Reply create limit & window seconds |
| `RL_IMAGE_LIMIT` / `RL_IMAGE_WINDOW` | No | Image upload limit & window seconds |
| `RUST_LOG` | No | Log level (info, debug, warn, error) |

### Development with VS Code

When using VS Code's debugger, environment variables are automatically loaded from the `.env` file via the launch configuration in `.vscode/launch.json`. 

To debug:
1. Ensure `.env` file exists with your configuration
2. Press `F5` or go to Run → Start Debugging
3. Select "Debug API (Rust)" or "Full-stack Dev"

### Running Without VS Code

Environment variables must be set before running the application:

```bash
# Linux/macOS - Set variables inline
JWT_SECRET="your-secret" RUST_LOG=info cargo run

# Or export them first
export JWT_SECRET="your-secret"
export RUST_LOG=info
cargo run

# Using a tool like direnv (install separately)
echo 'dotenv' > .envrc
direnv allow
cargo run
```

### Production

In production, set environment variables through your deployment platform:
- **Docker**: Use `--env-file .env` or individual `-e` flags
- **Kubernetes**: Use ConfigMaps and Secrets
- **systemd**: Use `Environment=` directives in service files
- **Cloud platforms**: Use their native environment configuration

### Bootstrap Admins
Set `BOOTSTRAP_ADMIN_DISCORD_IDS` (comma separated) to automatically grant Admin role to those Discord IDs on first login without needing a prior role assignment.