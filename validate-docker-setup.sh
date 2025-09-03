#!/bin/bash
# Docker setup validation script for RIB

set -e

echo "🔍 Validating RIB Docker setup..."

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed. Please install Docker first."
    exit 1
fi

# Check if Docker Compose is available
if ! command -v docker compose &> /dev/null && ! command -v docker-compose &> /dev/null; then
    echo "❌ Docker Compose is not installed. Please install Docker Compose first."
    exit 1
fi

echo "✅ Docker and Docker Compose are available"

# Validate docker-compose.yml
echo "🔍 Validating docker-compose configuration..."
if command -v docker compose &> /dev/null; then
    docker compose config --quiet
else
    docker-compose config --quiet
fi
echo "✅ Docker Compose configuration is valid"

# Check if .env file exists
if [[ ! -f .env ]]; then
    echo "⚠️  .env file not found. Creating from .env.example..."
    cp .env.example .env
    echo "📝 Please edit .env and set JWT_SECRET to a secure value (32+ characters)"
fi

# Validate .env file for required variables
if grep -q "JWT_SECRET=change-me-super-secret" .env; then
    echo "⚠️  Please update JWT_SECRET in .env file to a secure value (32+ characters)"
fi

# Check disk space (rough estimate: need ~2GB for images)
available_space=$(df . | tail -1 | awk '{print $4}')
if [[ $available_space -lt 2097152 ]]; then  # 2GB in KB
    echo "⚠️  Low disk space detected. Docker build may require ~2GB free space"
fi

echo ""
echo "🎉 Setup validation complete!"
echo ""
echo "To start RIB with Docker:"
echo "1. Edit .env and set JWT_SECRET to a secure value"
echo "2. Run: docker compose up -d"
echo "3. Access frontend at: http://localhost:3000"
echo "4. Access backend API at: http://localhost:8080"
echo "5. View API docs at: http://localhost:8080/docs"
echo ""
echo "For detailed instructions, see docs/DOCKER.md"