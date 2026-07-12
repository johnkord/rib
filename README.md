# RIB

RIB is a self-hostable, public-read media board with gated posting and private moderator accountability. It powers `rib.curlyquote.com` and is also intended to be reusable by other operators.

Posting admission currently supports:

- Explicitly allowlisted Discord identities
- An experimental Bitcoin proof-of-value path for pseudoanonymous posters

Posts do not expose login identities to ordinary readers. Moderators can privately resolve a post to its admission subject and ban that subject for abuse. Posters can opt into a public, reusable identity with a classic password tripcode.

Status: early-stage, pre-1.0. Run a private or controlled instance until the production checklist below is complete.

## Current Features

- Public boards, threads, replies, and attachments
- PostgreSQL persistence with forward SQLx migrations
- S3-compatible attachment storage, including MinIO
- Arbitrary file attachments up to 25 MiB
- Inline previews for deliberately supported media and downloads for other formats
- SHA-256 blob deduplication with multiple post references per blob
- Discord OAuth with signed state, PKCE, and HttpOnly session cookies
- Explicit Discord allowlisting through role assignments
- Bitcoin signed-message and confirmed-balance admission experiment
- Optional classic password tripcodes
- Private moderator-only author attribution
- Subject bans with optional expiry and reason
- Moderator soft-delete/restore and admin hard-delete workflows
- In-memory write rate limits for a single application replica
- Generated OpenAPI/Swagger UI, Prometheus metrics, structured request tracing, and security headers
- React SPA embedded in the Rust binary for same-origin deployment

## Product Model

RIB separates three concepts:

1. Admission proves that somebody may post. Discord subjects must be explicitly allowlisted. A qualifying Bitcoin proof is the alternate experimental path.
2. Public identity is optional. A poster may add a name and tripcode, but ordinary posts otherwise appear anonymous.
3. Moderation identity is private. Every post retains provider attribution that only moderators and admins can resolve for abuse enforcement.

A tripcode password is transformed with a dedicated, instance-scoped secret. RIB stores the derived tripcode, never the password. Reusing a tripcode intentionally makes posts publicly linkable.

Attachments on public posts are intended to remain public indefinitely. Soft deletion hides an attachment while retaining it for restoration. Hard deletion and legal takedown remove the final unreferenced object. Abandoned-upload expiry and asynchronous deletion retries remain planned work.

## Architecture

RIB is a modular monolith:

```text
Browser
  |
  v
Actix Web (API + embedded React SPA)
  |                    |
  v                    v
PostgreSQL       S3-compatible storage
```

Main components:

- `src/routes.rs`: HTTP handlers, admission checks, validation, moderation, and media delivery
- `src/repo.rs`: PostgreSQL repositories
- `src/auth.rs`: JWT/session, OAuth transaction, and role primitives
- `src/storage.rs`: S3/MinIO object storage
- `src/rate_limit.rs`: bounded in-process write limits
- `rib-react/`: React, TypeScript, TanStack Query, and Vite frontend
- `migrations/`: forward-only SQLx migrations
- `tests/`: API and repository integration tests

PostgreSQL and S3-compatible storage are required. Redis is deployed by some development and Kubernetes configurations but is not yet used by application code. Until challenges and rate limits move to shared state, run one backend replica.

## Requirements

- Rust 1.97.0, pinned by `rust-toolchain.toml`
- Node.js 20 or later
- PostgreSQL 16 or a compatible supported PostgreSQL release
- S3-compatible object storage, such as MinIO
- Docker with Compose for the easiest local setup

## Quick Start With Docker Compose

1. Create local configuration:

   ```bash
   cp .env.example .env
   ```

2. Replace at least these values in `.env` with independent random secrets:

   ```env
   JWT_SECRET=<32-or-more-random-characters>
   TRIPCODE_SECRET=<different-32-or-more-random-characters>
   ```

   Generate each value with:

   ```bash
   openssl rand -hex 32
   ```

3. Start the stack:

   ```bash
   docker compose up --build
   ```

4. Open:
   - Application: http://localhost:8080
   - API documentation: http://localhost:8080/docs
   - Metrics: http://localhost:8080/metrics
   - MinIO console: http://localhost:9001

The Compose credentials are development defaults. Do not expose this configuration publicly.

## Local Development

Start infrastructure:

```bash
docker compose up -d postgres redis minio
```

Copy and edit the environment file:

```bash
cp .env.example .env
```

Run the backend, which serves the currently embedded frontend:

```bash
cargo run
```

For live frontend development, use a second terminal:

```bash
cd rib-react
npm ci
npm run dev
```

The Vite app uses `http://localhost:8080` as its development API by default. The backend allows the local Vite origin.

To refresh assets embedded by local Rust builds:

```bash
cd rib-react
npm run build
rm -rf ../embedded-frontend/*
cp -r dist/* ../embedded-frontend/
cd ..
cargo build
```

The Dockerfile builds and embeds the frontend automatically.

## Authentication And Admission

### Discord

Discord OAuth authenticates an identity but does not grant posting by itself. An admin must add an explicit role assignment for:

```text
discord:<discord-user-id>
```

Valid assignments are `user`, `moderator`, and `admin`. A missing assignment is denied. IDs listed in `BOOTSTRAP_ADMIN_DISCORD_IDS` are the recovery exception and receive admin access during login.

Configure a Discord application with this callback for local development:

```text
http://localhost:8080/api/v1/auth/discord/callback
```

Then set `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET`, and `DISCORD_REDIRECT_URI`.

### Bitcoin Proof Of Value

The Bitcoin path is an anti-spam experiment for posters who are not Discord-allowlisted. The server issues a one-use challenge, verifies a supported Bitcoin signed message, and checks confirmed UTXOs through configured explorer APIs. The default threshold is 1,000,000 satoshis (0.01 BTC).

This is an admission signal, not proof of a unique person or permanent ownership. The address is private moderator attribution and is sent to third-party explorers during balance checks. See [docs/bitcoin-proof-of-value-auth.md](docs/bitcoin-proof-of-value-auth.md) for protocol background; the current Rust routes remain the source of truth.

### Sessions

Browser sessions use an HttpOnly, same-site cookie. Bearer JWT extraction remains supported for API compatibility. Set `COOKIE_SECURE=true` whenever the public origin uses HTTPS.

## Tripcodes

A poster may provide an optional display name and tripcode password. RIB derives a stable public tripcode with HMAC-SHA-256 and `TRIPCODE_SECRET`, then discards the password before persistence.

Operational rules:

- Never reuse `JWT_SECRET` as `TRIPCODE_SECRET` in production.
- Do not rotate `TRIPCODE_SECRET` casually. Rotation changes all newly derived tripcodes.
- Never log request bodies containing tripcode passwords.
- A tripcode is a voluntary public link between posts, not an account or authorization factor.

## Attachments

RIB intentionally accepts broad file types. Security behavior differs by delivery class:

- Supported images, video, and audio may be previewed.
- Active, unknown, archive, office, and other non-previewable content is downloaded as an attachment.
- MIME is detected from bytes rather than trusted from the multipart header.
- Public object URLs use validated 64-character SHA-256 hashes.
- One stored blob may be referenced by multiple posts.

Current limits and remaining work:

- Per-file maximum: 25 MiB
- Kubernetes ingress maximum: 25 MiB
- Upload and download currently buffer complete objects in application memory
- Malware quarantine/scanning, byte ranges, thumbnails, a separate media origin, and a retryable deletion worker are not yet implemented

Do not treat the current arbitrary-file pipeline as hardened for hostile public uploads until those controls are added.

## Moderation

Moderators and admins can:

- Resolve a thread or reply to its private admission subject
- Ban and unban subjects
- Record a ban reason and optional expiration
- Soft-delete and restore threads and replies

Admins can additionally:

- Create and update boards
- Manage role assignments
- Hard-delete boards, threads, and replies

Public thread and reply responses omit private attribution. A soft-deleted board also hides descendants reached through direct IDs.

## API And Operations

- API base: `/api/v1`
- OpenAPI/Swagger UI: `/docs`
- OpenAPI JSON: `/docs/openapi.json`
- Health: `/healthz`
- Prometheus metrics: `/metrics`
- Public attachments: `/images/{sha256}`

The generated OpenAPI document covers the main public, auth, role, ban, and moderation endpoints. The handler definitions are authoritative if documentation and behavior differ.

## Configuration

See [.env.example](.env.example) for the complete development template.

| Variable                      | Required                            | Purpose                                                              |
| ----------------------------- | ----------------------------------- | -------------------------------------------------------------------- |
| `JWT_SECRET`                  | Yes                                 | Signs sessions and OAuth transaction state; minimum 32 characters    |
| `TRIPCODE_SECRET`             | Yes for stable production tripcodes | Derives public tripcodes; use a separate minimum 32-character secret |
| `DATABASE_URL`                | Yes                                 | PostgreSQL connection URL                                            |
| `S3_ENDPOINT`                 | Yes                                 | S3 or MinIO endpoint                                                 |
| `S3_ACCESS_KEY`               | Provider-dependent                  | S3 access identity                                                   |
| `S3_SECRET_KEY`               | Provider-dependent                  | S3 secret                                                            |
| `S3_BUCKET`                   | No                                  | Bucket name; defaults to `rib-images`                                |
| `S3_REGION`                   | No                                  | Region; defaults to `us-east-1`                                      |
| `FRONTEND_URL`                | No                                  | Canonical SPA origin and OAuth redirect base                         |
| `COOKIE_SECURE`               | Production                          | Marks session and OAuth cookies secure                               |
| `DISCORD_CLIENT_ID`           | For Discord                         | Discord OAuth client ID                                              |
| `DISCORD_CLIENT_SECRET`       | For Discord                         | Discord OAuth client secret                                          |
| `DISCORD_REDIRECT_URI`        | For Discord                         | Exact registered callback URI                                        |
| `BOOTSTRAP_ADMIN_DISCORD_IDS` | Initial setup                       | Comma-separated recovery admin IDs                                   |
| `BTC_MIN_BALANCE_SATS`        | No                                  | Bitcoin threshold; defaults to 1,000,000                             |
| `BTC_BLOCKSTREAM_API_BASE`    | No                                  | Blockstream-compatible API base                                      |
| `RL_ENABLED`                  | Production                          | Enables application write limits                                     |
| `RL_*`                        | No                                  | Per-action limits and windows                                        |
| `TRUST_PROXY_HEADERS`         | Behind a trusted proxy              | Enables forwarded client-IP parsing                                  |
| `TRUSTED_PROXY_HOPS`          | With trusted proxy headers          | Number of trusted right-most proxy hops                              |
| `ENABLE_HSTS`                 | HTTPS production                    | Enables HSTS response header                                         |
| `RUST_LOG`                    | No                                  | Tracing filter                                                       |

`TRUST_PROXY_HEADERS` is safe only when the edge proxy strips or overwrites inbound forwarding headers.

## Testing And Quality Gates

Backend integration tests create data and intentionally fail when PostgreSQL is unavailable. The local wrapper creates, migrates, and drops a disposable database so the development database stays clean. It requires the SQLx CLI used by CI.

```bash
make test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Install the pinned migration CLI when needed:

```bash
cargo install sqlx-cli --version 0.8.6 --no-default-features --features rustls,postgres --locked
```

CI or other ephemeral environments may instead set `DATABASE_URL`, run `sqlx migrate run`, and invoke `cargo test` directly.

Frontend checks:

```bash
cd rib-react
npm ci
npm run lint
npm test -- --run
npm run build
npm audit --audit-level=high
```

CI also builds the embedded frontend, applies migrations, validates all Kustomize overlays, builds Bicep, and builds the Rust release binary.

## Deployment

### Small Or Private Instance

Use Docker Compose with operator-supplied secrets and backups. The checked-in development defaults are not production credentials.

### Kubernetes / AKS

Kustomize overlays are under `k8s/overlays/`. See [k8s/README.md](k8s/README.md) and the [production release runbook](docs/production-release.md). The repository currently caps the backend at one replica because Bitcoin challenges and application rate limits are process-local.

The existing live AKS instance was found to use single-replica in-cluster PostgreSQL and MinIO on one old Kubernetes node. Repository fixes do not modify that live infrastructure. Before upgrading or redeploying it:

1. Create off-cluster PostgreSQL and MinIO backups.
2. Prove those backups restore.
3. Rotate PostgreSQL and MinIO root credentials.
4. Give the application least-privilege database and bucket identities.
5. Upgrade AKS through a supported sequence.
6. Add network policy and recovery monitoring.

For a new public deployment, prefer managed PostgreSQL and object storage unless the operator is prepared to own database and object-store backup, restore, patching, and availability.

## Production Checklist

Before public exposure:

- Configure exact public origins and secure cookies.
- Configure Discord allowlisting and at least one recoverable admin.
- Use independent random JWT and tripcode secrets from a secret manager.
- Use least-privilege PostgreSQL and object-storage credentials.
- Enable trusted-proxy handling only behind a sanitizing proxy.
- Enable application and edge rate limits.
- Add malware quarantine/scanning for arbitrary uploads.
- Put user files on a cookieless media origin.
- Establish and test database/object backups.
- Add privacy, content, reporting, retention, and takedown policies.
- Run all CI quality and security gates.
- Keep one backend replica until ephemeral state is distributed.

## Known Limitations

- No cursor pagination or search
- No report queue, appeal workflow, or moderation audit log
- No upload quarantine or malware scanning
- No streaming upload/download, range requests, thumbnails, or CDN integration
- No distributed rate limits or shared Bitcoin challenge state
- No server-side session revocation list; privileged claims remain usable until token expiry
- No broad browser end-to-end suite
- No automated backup or restore workflow

The detailed baseline review and implementation status are in [docs/repository-review-2026-07-11.md](docs/repository-review-2026-07-11.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Keep changes focused, add negative-path coverage for API behavior, and run both Rust and frontend gates before opening a pull request.

## License

MIT. See [LICENSE](LICENSE).
