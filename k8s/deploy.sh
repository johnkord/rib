#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
NAMESPACE="rib"
REGISTRY=""
DOMAIN="rib.example.com"
BUILD_IMAGES="false"
SKIP_INGRESS="false"
SKIP_HPA="false"

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

# Function to check prerequisites
check_prerequisites() {
    print_status "Checking prerequisites..."
    
    # Check kubectl
    if ! command -v kubectl &> /dev/null; then
        print_error "kubectl is not installed or not in PATH"
        exit 1
    fi
    
    # Check if kubectl can connect to cluster
    if ! kubectl cluster-info &> /dev/null; then
        print_error "Cannot connect to Kubernetes cluster. Please configure kubectl."
        exit 1
    fi
    
    # Check for NGINX Ingress Controller
    if kubectl get pods -n ingress-nginx -l app.kubernetes.io/name=ingress-nginx &> /dev/null; then
        print_success "NGINX Ingress Controller found"
    else
        print_warning "NGINX Ingress Controller not found. Ingress may not work."
        print_warning "Install with: kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-v1.8.2/deploy/static/provider/cloud/deploy.yaml"
    fi
    
    # Check for Metrics Server (needed for HPA)
    if kubectl get deployment metrics-server -n kube-system &> /dev/null; then
        print_success "Metrics Server found"
    else
        print_warning "Metrics Server not found. HPA will not work."
        print_warning "Install with: kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml"
    fi
}

# Function to build and push images
build_images() {
    if [ "$BUILD_IMAGES" = "true" ]; then
        print_status "Building container images..."
        
        # Build backend image
        print_status "Building backend image..."
        docker build -t rib:latest .
        
        # Build frontend image
        print_status "Building frontend image..."
        docker build -t rib-frontend:latest ./rib-react
        
        if [ -n "$REGISTRY" ]; then
            print_status "Tagging and pushing images to registry..."
            
            # Tag and push backend
            docker tag rib:latest $REGISTRY/rib:latest
            docker push $REGISTRY/rib:latest
            
            # Tag and push frontend
            docker tag rib-frontend:latest $REGISTRY/rib-frontend:latest
            docker push $REGISTRY/rib-frontend:latest
            
            print_success "Images pushed to registry"
        else
            print_warning "No registry specified. Images available locally only."
        fi
    fi
}

# Function to update image references
update_image_references() {
    if [ -n "$REGISTRY" ]; then
        print_status "Updating image references to use registry..."
        
        # Create temporary directory for modified files
        TMP_DIR=$(mktemp -d)
        cp -r k8s/* $TMP_DIR/
        
        # Update backend image reference
        sed -i "s|image: rib:latest|image: $REGISTRY/rib:latest|" $TMP_DIR/rib-backend.yaml
        
        # Update frontend image reference
        sed -i "s|image: rib-frontend:latest|image: $REGISTRY/rib-frontend:latest|" $TMP_DIR/rib-frontend.yaml
        
        # Use temporary directory for deployment
        K8S_DIR=$TMP_DIR
    else
        K8S_DIR="k8s"
    fi
}

# Function to update domain in ingress
update_domain() {
    if [ "$DOMAIN" != "rib.example.com" ]; then
        print_status "Updating domain to $DOMAIN..."
        sed -i "s|host: rib.example.com|host: $DOMAIN|" $K8S_DIR/ingress.yaml
    fi
}

# Function to deploy infrastructure
deploy_infrastructure() {
    print_status "Deploying infrastructure components..."
    
    # Create namespace
    print_status "Creating namespace..."
    kubectl apply -f $K8S_DIR/namespace.yaml
    
    # Create storage
    print_status "Creating persistent volumes..."
    kubectl apply -f $K8S_DIR/storage.yaml
    
    # Create configuration
    print_status "Creating configuration..."
    kubectl apply -f $K8S_DIR/configmap.yaml
    
    # Deploy databases and supporting services
    print_status "Deploying PostgreSQL..."
    kubectl apply -f $K8S_DIR/postgres.yaml
    
    print_status "Deploying Redis..."
    kubectl apply -f $K8S_DIR/redis.yaml
    
    print_status "Deploying MinIO..."
    kubectl apply -f $K8S_DIR/minio.yaml
    
    print_status "Waiting for infrastructure to be ready..."
    
    # Wait for PostgreSQL
    print_status "Waiting for PostgreSQL..."
    kubectl wait --for=condition=ready pod -l component=postgres -n $NAMESPACE --timeout=300s
    
    # Wait for Redis
    print_status "Waiting for Redis..."
    kubectl wait --for=condition=ready pod -l component=redis -n $NAMESPACE --timeout=300s
    
    # Wait for MinIO
    print_status "Waiting for MinIO..."
    kubectl wait --for=condition=ready pod -l component=minio -n $NAMESPACE --timeout=300s
    
    print_success "Infrastructure deployed successfully"
}

# Function to deploy applications
deploy_applications() {
    print_status "Deploying application components..."
    
    # Deploy backend
    print_status "Deploying RIB backend..."
    kubectl apply -f $K8S_DIR/rib-backend.yaml
    
    # Deploy frontend
    print_status "Deploying RIB frontend..."
    kubectl apply -f $K8S_DIR/rib-frontend.yaml
    
    print_status "Waiting for applications to be ready..."
    
    # Wait for backend
    print_status "Waiting for backend..."
    kubectl wait --for=condition=ready pod -l component=backend -n $NAMESPACE --timeout=300s
    
    # Wait for frontend
    print_status "Waiting for frontend..."
    kubectl wait --for=condition=ready pod -l component=frontend -n $NAMESPACE --timeout=300s
    
    print_success "Applications deployed successfully"
}

# Function to deploy ingress
deploy_ingress() {
    if [ "$SKIP_INGRESS" = "false" ]; then
        print_status "Deploying Ingress..."
        kubectl apply -f $K8S_DIR/ingress.yaml
        print_success "Ingress deployed successfully"
        
        print_status "Getting Ingress information..."
        kubectl get ingress -n $NAMESPACE
    else
        print_warning "Skipping Ingress deployment"
    fi
}

# Function to deploy HPA
deploy_hpa() {
    if [ "$SKIP_HPA" = "false" ]; then
        print_status "Deploying Horizontal Pod Autoscaler..."
        kubectl apply -f $K8S_DIR/hpa.yaml
        print_success "HPA deployed successfully"
        
        print_status "Getting HPA status..."
        kubectl get hpa -n $NAMESPACE
    else
        print_warning "Skipping HPA deployment"
    fi
}

# Function to show deployment status
show_status() {
    print_status "Deployment Status:"
    echo
    
    print_status "Pods:"
    kubectl get pods -n $NAMESPACE
    echo
    
    print_status "Services:"
    kubectl get services -n $NAMESPACE
    echo
    
    if [ "$SKIP_INGRESS" = "false" ]; then
        print_status "Ingress:"
        kubectl get ingress -n $NAMESPACE
        echo
    fi
    
    if [ "$SKIP_HPA" = "false" ]; then
        print_status "HPA:"
        kubectl get hpa -n $NAMESPACE
        echo
    fi
    
    print_success "Deployment completed successfully!"
    echo
    print_status "To access the application:"
    if [ "$SKIP_INGRESS" = "false" ]; then
        echo "  - Frontend: http://$DOMAIN"
        echo "  - API: http://$DOMAIN/api"
        echo "  - Docs: http://$DOMAIN/docs"
    else
        echo "  - Use 'kubectl port-forward' to access services:"
        echo "    kubectl port-forward -n $NAMESPACE service/rib-frontend-service 3000:80"
        echo "    kubectl port-forward -n $NAMESPACE service/rib-backend-service 8080:8080"
    fi
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -r, --registry REGISTRY    Container registry to push images to"
    echo "  -d, --domain DOMAIN        Domain name for ingress (default: rib.example.com)"
    echo "  -b, --build                Build and push container images"
    echo "  --skip-ingress             Skip ingress deployment"
    echo "  --skip-hpa                 Skip HPA deployment"
    echo "  -h, --help                 Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 --build --registry myregistry.com --domain rib.mycompany.com"
    echo "  $0 --skip-ingress --skip-hpa"
    echo "  $0 --domain localhost"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        -d|--domain)
            DOMAIN="$2"
            shift 2
            ;;
        -b|--build)
            BUILD_IMAGES="true"
            shift
            ;;
        --skip-ingress)
            SKIP_INGRESS="true"
            shift
            ;;
        --skip-hpa)
            SKIP_HPA="true"
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

# Main execution
print_status "Starting RIB Kubernetes deployment..."
print_status "Registry: ${REGISTRY:-"local only"}"
print_status "Domain: $DOMAIN"
print_status "Build images: $BUILD_IMAGES"

check_prerequisites
build_images
update_image_references
update_domain
deploy_infrastructure
deploy_applications
deploy_ingress
deploy_hpa
show_status

# Cleanup temporary directory if created
if [ -n "$TMP_DIR" ]; then
    rm -rf $TMP_DIR
fi