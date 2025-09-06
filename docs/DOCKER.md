# Docker Deployment Guide

This guide explains how to run RIB using Docker and Docker Compose.

## Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- At least 2GB free disk space
- At least 1GB free RAM

## Quick Start

1. **Clone the repository:**
   ```bash
   git clone https://github.com/yourusername/rib.git
   cd rib
   ```

2. **Set up environment variables:**
   ```bash
   cp .env.example .env
   ```
   
   Edit `.env` and set at least the `JWT_SECRET` to a secure value (minimum 32 characters):
   ```
   JWT_SECRET=your-super-secret-jwt-key-at-least-32-characters-long
   ```

3. **Start all services:**
   ```bash
   docker-compose up -d
   # OR using the provided Makefile
   make docker-up
   ```

4. **Wait for services to be ready:**
   ```bash
   docker-compose ps
   ```

5. **Access the application:**
   - **Frontend**: http://localhost:3000
   - **Backend API**: http://localhost:8080
   - **API Documentation**: http://localhost:8080/docs
   - **MinIO Console**: http://localhost:9001 (admin/admin)

## Services Overview

| Service | Port | Purpose |
|---------|------|---------|
| `rib-frontend` | 3000 | React frontend with Nginx |
| `rib-backend` | 8080 | Rust API server |
| `postgres` | 5432 | PostgreSQL database |
| `redis` | 6379 | Redis cache |
| `minio` | 9000/9001 | S3-compatible object storage |

## Environment Variables

The application uses the following environment variables (see `.env.example`):

### Required
- `JWT_SECRET`: Secret key for JWT tokens (minimum 32 characters)

### Optional
- `RUST_LOG`: Log level (default: info)
- `FRONTEND_URL`: Frontend URL for CORS (default: http://localhost:3000)
- `DISCORD_CLIENT_ID`: Discord OAuth app ID
- `DISCORD_CLIENT_SECRET`: Discord OAuth secret
- `BOOTSTRAP_ADMIN_DISCORD_IDS`: Comma-separated Discord user IDs for bootstrap admins

## Storage

Data is persisted in Docker volumes:
- `postgres-data`: Database data
- `redis-data`: Redis data
- `minio-data`: Object storage data
- `rib-data`: Application data (in-memory store snapshots)

## Development Mode

For development with hot reload:

1. **Start dependencies only:**
   ```bash
   docker-compose up -d postgres redis minio
   # OR using the provided Makefile
   make docker-dev
   ```

2. **Run backend locally:**
   ```bash
   cargo run
   ```

3. **Run frontend locally:**
   ```bash
   cd rib-react
   npm run dev
   ```

## Production Deployment

For production deployment:

1. **Set production environment variables in `.env`:**
   ```bash
   JWT_SECRET=your-secure-production-secret
   RUST_LOG=warn
   ```

2. **Use production compose file:**
   ```bash
   docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
   ```

## Troubleshooting

### Services not starting
```bash
# Check service status
docker-compose ps

# View logs for a specific service
docker-compose logs rib-backend
docker-compose logs rib-frontend

# View logs for all services
docker-compose logs
```

### Database connection issues
```bash
# Check if PostgreSQL is ready
docker-compose exec postgres pg_isready -U postgres

# Connect to database
docker-compose exec postgres psql -U postgres -d rib
```

### Storage issues
```bash
# Check MinIO status
docker-compose exec minio mc admin info local

# Create bucket if missing
docker-compose exec minio mc mb local/rib-images
```

### Reset all data
```bash
# Stop services and remove volumes
docker-compose down -v

# Start fresh
docker-compose up -d
```

## Building Images Locally

If you need to build the Docker images locally:

```bash
# Build backend image
docker build -t rib-backend .

# Build frontend image
cd rib-react
docker build -t rib-frontend .
```

## Cross-Platform Compatibility

This Docker setup works on:
- ✅ Linux (amd64, arm64)
- ✅ macOS (Intel, Apple Silicon)
- ✅ Windows (WSL2, Docker Desktop)

## Security Considerations

For production use:
- Change default passwords for PostgreSQL, MinIO
- Use proper secrets management
- Set up HTTPS termination with a reverse proxy
- Configure firewall rules
- Regular security updates

## Monitoring

Health checks are configured for all services:
```bash
# Check health status
docker-compose ps
```

## Backup

To backup your data:
```bash
# Backup database
docker-compose exec postgres pg_dump -U postgres rib > backup.sql

# Backup volumes
docker run --rm -v rib_postgres-data:/data -v $(pwd):/backup alpine tar czf /backup/postgres-backup.tar.gz -C /data .
```