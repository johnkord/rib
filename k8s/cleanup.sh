#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

NAMESPACE="rib"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Remove RIB Kubernetes deployment"
    echo ""
    echo "Options:"
    echo "  --keep-data    Keep persistent volumes (don't delete data)"
    echo "  -h, --help     Show this help message"
}

# Parse command line arguments
KEEP_DATA="false"
while [[ $# -gt 0 ]]; do
    case $1 in
        --keep-data)
            KEEP_DATA="true"
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

print_status "Removing RIB Kubernetes deployment..."

# Check if namespace exists
if ! kubectl get namespace $NAMESPACE &> /dev/null; then
    print_warning "Namespace $NAMESPACE does not exist"
    exit 0
fi

# Remove HPA
print_status "Removing Horizontal Pod Autoscalers..."
kubectl delete -f k8s/hpa.yaml --ignore-not-found=true

# Remove Ingress
print_status "Removing Ingress..."
kubectl delete -f k8s/ingress.yaml --ignore-not-found=true

# Remove applications
print_status "Removing applications..."
kubectl delete -f k8s/rib-backend.yaml --ignore-not-found=true
kubectl delete -f k8s/rib-frontend.yaml --ignore-not-found=true

# Remove infrastructure
print_status "Removing infrastructure..."
kubectl delete -f k8s/postgres.yaml --ignore-not-found=true
kubectl delete -f k8s/redis.yaml --ignore-not-found=true
kubectl delete -f k8s/minio.yaml --ignore-not-found=true

# Remove configuration
print_status "Removing configuration..."
kubectl delete -f k8s/configmap.yaml --ignore-not-found=true

# Remove storage (optional)
if [ "$KEEP_DATA" = "false" ]; then
    print_status "Removing persistent volumes..."
    kubectl delete -f k8s/storage.yaml --ignore-not-found=true
else
    print_warning "Keeping persistent volumes (--keep-data specified)"
fi

# Remove namespace
print_status "Removing namespace..."
kubectl delete -f k8s/namespace.yaml --ignore-not-found=true

print_success "RIB deployment removed successfully!"

if [ "$KEEP_DATA" = "true" ]; then
    echo
    print_warning "Persistent volumes were kept. To remove them manually:"
    echo "  kubectl delete pvc -l app=rib"
fi