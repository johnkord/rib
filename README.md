# RIB – Rust Image Board

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
- Video support

### 1.4 Glossary
Thread – top-level post  
Reply – comment in a thread  
Board – category of threads  
Bump – reply that lifts a thread  
Sage – non-bumping reply

## 2. Architecture
Single Actix-web service backed by:
• pluggable RDBMS (PostgreSQL, SQLite)  
• optional Redis cache  
• S3-compatible object store for images

Key layers: API → Service → Repository → Storage/Cache.

## 3. Data Models
See `src/models.rs` for full Rust definitions:
Board, Thread, Reply, Image, Report (+ enums).

Validation highlights:  
• board.slug: `^[a-z0-9_]{1,12}$`  
• post.content: 1-2000 chars  
• image ≤ 10 MB, types: png/jpeg/gif/webp

## 4. API
OpenAPI spec: `/docs/api/openapi.yaml`  
Major groups: boards, threads, replies, images, moderation.  
Supports idempotency (header `Idempotency-Key`) and versioned base path `/api/v1`.

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

### 5.3 Image Storage
- **Primary**: S3-compatible object storage (AWS S3, MinIO, etc.)
- **CDN**: CloudFlare or similar for global distribution
- **Thumbnails**: Generated on upload, stored separately
- **Deduplication**: SHA256 hash checking before storage

### 5.4 Backup & Disaster Recovery
- **Database**: Point-in-time recovery (WAL shipping to cold storage).  
- **Images**: Cross-region replication in object storage.  
- **Config**: Encrypted off-site backups of `.env` and Kubernetes secrets.  
- Quarterly recovery drills verify backup integrity.

### 5.5 Data Retention & Lifecycle
| Data | Retention | Action |
|------|-----------|--------|
| Threads (active) | indefinite | None |
| Threads (archived) | 365 days default | Purge images, keep metadata |
| Reports | 180 days | Hard delete |
| Audit logs | 730 days | Glacier/offline archive |
| Hashed IPs | 90 days | Rotating re-hash + purge old salts |

### 5.6 Image Processing Pipeline
1. Upload received (streamed, max size enforced early).
2. MIME sniff + magic number verify.
3. Hash (SHA256) while streaming -> dedupe check.
4. Temporary quarantine storage.
5. Optional scanning (clamd / external API).
6. Resize + thumbnail (libvips recommended).
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
- Per-IP rate limits:
  - 1 thread per 5 minutes
  - 10 replies per minute
  - 5 images per hour
- Exponential backoff for repeated violations

### 6.3 Content Security
- **CAPTCHA**: Required for thread creation
- **Spam detection**: Bayesian filter for common spam patterns
- **Image validation**: File type verification, virus scanning
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
Pluggable hook that sends image hashes to third-party services
(e.g., PhotoDNA) before the file is made public. Failing images are quarantined.

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
Token bucket in Redis:
- Key pattern: `rl:{ip}:{scope}`
- Lua script for atomic check/decrement
- BURST & REFILL configurable per scope (thread/reply/image)

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
- Runbooks stored in the repository `/docs/runbooks/*.md`.  
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

**Option A: Full Docker setup**
```bash
# Start all services with Docker Compose
docker-compose up -d

# View logs
docker-compose logs -f rib-backend
```

**Option B: Hybrid development** (recommended for active development)
```bash
# Start dependencies only
docker-compose up -d postgres redis minio mailhog

# Run backend locally with hot reload
cargo watch -x "run --features inmem-store"

# In another terminal, run frontend
cd rib-react
npm run dev
```

**Option C: Manual setup**
```bash
# Start dependencies
docker-compose up -d postgres redis minio

# Run migrations (if using postgres-store)
sqlx migrate run

# Start dev server
cargo run --features inmem-store
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
- `RIB_FEATURES="inmem-store,unsafe-fast-hash"` aids rapid prototyping (never in prod).

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
• Auth: JWT (stored in `HttpOnly` cookie) – shares secret with API  
• Build: Vite (bundles, hot reload), ESLint + Prettier

### 17.3 Core Pages  
| Route | Description |
|-------|-------------|
| `/` | Global board list |
| `/b/{slug}` | Thread catalog for a board |
| `/thread/{id}` | Thread view with infinite scroll replies |
| `/thread/new` | Create thread (CAPTCHA, image upload) |
| `/reply/{threadId}` | Quick-reply modal |
| `/mod` | Moderator dashboard (role-gated) |

### 17.4 Components  
- `ImageUpload` – drag-and-drop, progress bar, MIME/size pre-check  
- `PostForm` – markdown editor with live preview  
- `ThreadCard` – board catalog entry  
- `Reply` – collapsible with quote-link parsing  
- `AuthGuard` – protects moderator routes

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
Phase 1 (v0.1) – read-only pages  
Phase 2 (v0.5) – posting flows + image upload  
Phase 3 (v1.0) – moderation UI, live updates via WS

## Getting Started

### Prerequisites
- Rust 1.75+
- Node.js 18+ (for frontend)
- Docker & Docker Compose (recommended for full setup)

### Quick Start with Docker (Recommended)

The fastest way to get RIB running with all dependencies:

```bash
git clone https://github.com/yourusername/rib.git
cd rib

# Set up environment
cp .env.example .env
# Edit .env and set JWT_SECRET to a secure value (32+ characters)

# Start all services
docker-compose up -d

# Access the application
# Frontend: http://localhost:3000
# Backend API: http://localhost:8080
# API Docs: http://localhost:8080/docs
```

See [Docker Deployment Guide](docs/DOCKER.md) for detailed instructions.

### Manual Development Setup

1. Clone the repository:
```bash
git clone https://github.com/yourusername/rib.git
cd rib
```

2. Set up environment variables:

   **Option A: Using VS Code (Recommended for development)**
   - Copy `.env.example` to `.env`
   - Edit `.env` with your configuration
   - VS Code will automatically load these when debugging (F5)

   **Option B: Manual shell setup**
   ```bash
   # Linux/macOS
   export JWT_SECRET="your-very-long-secret-key-at-least-32-chars"
   export DISCORD_CLIENT_ID="your-discord-app-id"
   export DISCORD_CLIENT_SECRET="your-discord-secret"
   # ... other variables
   
   # Windows (PowerShell)
   $env:JWT_SECRET="your-very-long-secret-key-at-least-32-chars"
   $env:DISCORD_CLIENT_ID="your-discord-app-id"
   $env:DISCORD_CLIENT_SECRET="your-discord-secret"
   # ... other variables
   
   # Or source from .env file manually (Linux/macOS)
   export $(cat .env | xargs)
   ```

3. Configure Discord OAuth (optional):
   - Go to https://discord.com/developers/applications
   - Create a new application
   - Go to OAuth2 section
   - Add redirect URI: `http://localhost:8080/api/v1/auth/discord/callback`
   - Set `DISCORD_CLIENT_ID` and `DISCORD_CLIENT_SECRET` environment variables

4. Run the server:
```bash
cargo run --features inmem-store
```

5. Start the frontend (in another terminal):
```bash
cd rib-react
npm install
npm run dev
```

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `JWT_SECRET` | Yes | Secret key for JWT signing (min 32 chars) |
| `DISCORD_CLIENT_ID` | No | Discord OAuth application ID |
| `DISCORD_CLIENT_SECRET` | No | Discord OAuth application secret |
| `DISCORD_REDIRECT_URI` | No | OAuth callback URL |
| `FRONTEND_URL` | No | Frontend URL for redirects (default: http://localhost:5173) |
| `RIB_DATA_DIR` | No | Directory for in-memory snapshot (default: ./data) |
| `BOOTSTRAP_ADMIN_DISCORD_IDS` | No | Comma‑separated Discord user IDs granted Admin on first login (e.g. `188880431955968000`) |
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
JWT_SECRET="your-secret" RUST_LOG=info cargo run --features inmem-store

# Or export them first
export JWT_SECRET="your-secret"
export RUST_LOG=info
cargo run --features inmem-store

# Using a tool like direnv (install separately)
echo 'dotenv' > .envrc
direnv allow
cargo run --features inmem-store
```

### Production

In production, set environment variables through your deployment platform:
- **Docker**: Use `--env-file .env` or individual `-e` flags
- **Kubernetes**: Use ConfigMaps and Secrets
- **systemd**: Use `Environment=` directives in service files
- **Cloud platforms**: Use their native environment configuration

### Bootstrap Admins
Set `BOOTSTRAP_ADMIN_DISCORD_IDS` (comma separated) to automatically grant Admin role to those Discord IDs on first login without needing a prior role assignment.