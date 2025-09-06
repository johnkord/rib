# Multi-stage Dockerfile for RIB Rust backend
# Requires Rust >=1.82 due to ICU (unicode) crates pulled by dependencies
FROM rust:1.83 AS builder

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

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build the application (Postgres backend is default now)
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r rib && useradd -r -g rib rib

# Create app directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/rib /usr/local/bin/rib

# Copy migrations for database setup
COPY --from=builder /app/migrations ./migrations

# Create data directory with proper permissions
RUN mkdir -p /app/data && chown -R rib:rib /app

# Switch to non-root user
USER rib

# Expose the application port
EXPOSE 8080

# Health check - using wget instead of curl for smaller footprint
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/docs || exit 1

# Run the application
CMD ["rib"]