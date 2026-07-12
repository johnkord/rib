# Multi-stage Dockerfile for RIB Rust backend
# Requires Rust >=1.82 due to ICU (unicode) crates pulled by dependencies
FROM rust:1.97.0-bookworm AS builder

# Always embed frontend now; build arg retained for compatibility but ignored.
ARG EMBED_FRONTEND=true
ENV EMBED_FRONTEND=${EMBED_FRONTEND}

# Install necessary system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy dependency manifests first for better caching
COPY Cargo.toml Cargo.lock ./

COPY src ./src
COPY migrations ./migrations

# Optionally build frontend and place artifacts where Rust build can embed them.
# We do this prior to compiling the Rust binary so that "include_bytes!" / rust-embed can capture dist content.
FROM node:20 AS frontend-builder
WORKDIR /fe
COPY rib-react/package.json rib-react/package-lock.json* rib-react/tsconfig.json rib-react/vite.config.ts rib-react/tailwind.config.cjs rib-react/postcss.config.cjs ./
COPY rib-react/src ./src
COPY rib-react/index.html ./
RUN npm ci --no-audit --no-fund && npm run build

FROM builder AS builder-with-frontend
COPY --from=frontend-builder /fe/dist /app/embedded-frontend

# Back to primary builder stage for conditional copy via multi-stage logic.
FROM builder AS compile
COPY --from=builder-with-frontend /app/embedded-frontend /app/embedded-frontend

# Ensure an environment flag so build.rs (future) or code can detect embedding.
ENV RIB_EMBED_FRONTEND=true

# Build the application while retaining Cargo artifacts across BuildKit runs.
# The final copy moves the binary out of the cache mount into the image layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    echo "Building with embed-frontend feature (always on)" && \
    cargo build --release --features embed-frontend && \
    cp /app/target/release/rib /app/rib

# Runtime stage
## Runtime stage (match builder glibc by using the same base image family)
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        wget \
    && rm -rf /var/lib/apt/lists/* /usr/share/doc /usr/share/man

# Create non-root user
RUN groupadd -r rib && useradd -r -g rib rib

# Create app directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=compile /app/rib /usr/local/bin/rib

# Copy migrations for database setup
COPY --from=compile /app/migrations ./migrations

# Create data directory with proper permissions
RUN mkdir -p /app/data && chown -R rib:rib /app

# Switch to non-root user
USER rib

# Expose the application port
EXPOSE 8080

# Health check - using wget instead of curl for smaller footprint
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --output-document=/dev/null http://127.0.0.1:8080/healthz || exit 1

# Run the application
CMD ["rib"]