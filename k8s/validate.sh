#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to validate YAML syntax
validate_yaml() {
    local file=$1
    
    # Try kubectl dry-run first (if cluster is available)
    if kubectl cluster-info &> /dev/null; then
        if kubectl apply --dry-run=client -f "$file" &> /dev/null; then
            print_success "✓ $file (kubectl validation)"
            return 0
        else
            print_error "✗ $file (kubectl validation failed)"
            kubectl apply --dry-run=client -f "$file"
            return 1
        fi
    else
        # Fallback to basic YAML syntax check using Python
        if command -v python3 &> /dev/null; then
            if python3 -c "import yaml; yaml.safe_load_all(open('$file'))" &> /dev/null; then
                print_success "✓ $file (YAML syntax)"
                return 0
            else
                print_error "✗ $file (YAML syntax error)"
                python3 -c "import yaml; yaml.safe_load_all(open('$file'))"
                return 1
            fi
        else
            # Basic file existence check as last resort
            if [[ -f "$file" && -s "$file" ]]; then
                print_success "✓ $file (file exists)"
                return 0
            else
                print_error "✗ $file (file missing or empty)"
                return 1
            fi
        fi
    fi
}

print_status "Validating Kubernetes manifests..."

# Check if kubectl is available and can connect to cluster
KUBECTL_AVAILABLE=false
if command -v kubectl &> /dev/null && kubectl cluster-info &> /dev/null; then
    KUBECTL_AVAILABLE=true
    print_status "Using kubectl for validation"
elif command -v python3 &> /dev/null; then
    print_status "Using Python YAML parser for validation"
else
    print_status "Limited validation - checking file existence only"
fi

# Validate all YAML files
errors=0
for file in k8s/*.yaml; do
    if [[ -f "$file" ]]; then
        if ! validate_yaml "$file"; then
            ((errors++))
        fi
    fi
done

echo

if [ $errors -eq 0 ]; then
    print_success "All manifests are valid!"
    
    print_status "Checking for common issues..."
    
    # Check for placeholder values that need to be replaced
    if grep -r "rib.example.com" k8s/ &> /dev/null; then
        print_status "Note: Replace 'rib.example.com' with your actual domain in k8s/ingress.yaml"
    fi
    
    if grep -r "your-registry" k8s/ &> /dev/null; then
        print_status "Note: Update image references in deployment files to use your container registry"
    fi
    
    # Check for base64 encoded secrets
    if grep -q "eW91ci1zdXBlci1zZWNyZXQtand0LWtleS1hdC1sZWFzdC0zMi1jaGFyYWN0ZXJz" k8s/configmap.yaml; then
        print_status "Note: Update JWT_SECRET in k8s/configmap.yaml with your own secure secret"
    fi
    
    echo
    print_success "Kubernetes manifests validation completed successfully!"
    print_status "Use './k8s/deploy.sh --help' for deployment instructions"
else
    print_error "Found $errors validation errors. Please fix them before deploying."
    exit 1
fi