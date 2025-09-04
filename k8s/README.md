# Kubernetes Deployment for RIB (Rust Image Board)

This directory contains Kubernetes manifests for deploying the RIB application stack to a Kubernetes cluster.

## Prerequisites

1. **Kubernetes cluster** with kubectl configured
2. **NGINX Ingress Controller** installed (for Ingress support)
3. **Metrics Server** installed (for HPA support)
4. **Container images** built and pushed to a registry:
   - `rib:latest` (backend application)
   - `rib-frontend:latest` (frontend application)

## Quick Start

### 1. Build and Push Container Images

First, build the application images:

```bash
# Build backend image
docker build -t rib:latest .

# Build frontend image  
docker build -t rib-frontend:latest ./rib-react

# Tag and push to your registry (replace with your registry)
docker tag rib:latest your-registry/rib:latest
docker tag rib-frontend:latest your-registry/rib-frontend:latest
docker push your-registry/rib:latest
docker push your-registry/rib-frontend:latest
```

### 2. Update Image References

Update the image references in the deployment files to point to your registry:

```bash
# Update rib-backend.yaml
sed -i 's|image: rib:latest|image: your-registry/rib:latest|' k8s/rib-backend.yaml

# Update rib-frontend.yaml  
sed -i 's|image: rib-frontend:latest|image: your-registry/rib-frontend:latest|' k8s/rib-frontend.yaml
```

### 3. Configure Secrets

Update the secrets in `k8s/configmap.yaml`:

```bash
# Generate a secure JWT secret (base64 encoded)
echo -n "your-super-secret-jwt-key-at-least-32-characters" | base64

# Update other secrets as needed for Discord OAuth, etc.
```

### 4. Configure Domain

Update the Ingress configuration in `k8s/ingress.yaml`:

```yaml
spec:
  rules:
  - host: your-domain.com  # Replace with your actual domain
```

### 5. Deploy to Kubernetes

Deploy all components in order:

```bash
# Create namespace
kubectl apply -f k8s/namespace.yaml

# Create storage
kubectl apply -f k8s/storage.yaml

# Create configuration
kubectl apply -f k8s/configmap.yaml

# Deploy databases and services
kubectl apply -f k8s/postgres.yaml
kubectl apply -f k8s/redis.yaml
kubectl apply -f k8s/minio.yaml

# Wait for dependencies to be ready
kubectl wait --for=condition=ready pod -l component=postgres -n rib --timeout=300s
kubectl wait --for=condition=ready pod -l component=redis -n rib --timeout=300s
kubectl wait --for=condition=ready pod -l component=minio -n rib --timeout=300s

# Deploy applications
kubectl apply -f k8s/rib-backend.yaml
kubectl apply -f k8s/rib-frontend.yaml

# Create ingress for external access
kubectl apply -f k8s/ingress.yaml

# Optional: Enable auto-scaling
kubectl apply -f k8s/hpa.yaml
```

## Architecture

The deployment consists of:

### Core Services
- **rib-backend**: Rust application (3 replicas by default)
- **rib-frontend**: React application served by Nginx (2 replicas by default)

### Supporting Services  
- **postgres**: PostgreSQL database (1 replica)
- **redis**: Redis cache (1 replica)
- **minio**: S3-compatible object storage (1 replica)

### Networking
- **Ingress**: Routes external traffic to frontend and API
- **Services**: Internal cluster networking between components

### Storage
- **PersistentVolumeClaims**: Persistent storage for databases and application data

### Auto-scaling
- **HorizontalPodAutoscaler**: Automatically scales frontend and backend based on CPU/memory usage

## Configuration

### Environment Variables

All configuration is managed through ConfigMaps and Secrets:

- **rib-config** (ConfigMap): Non-sensitive configuration
- **rib-secrets** (Secret): Sensitive data like JWT secrets, database passwords

### Storage

Default storage allocations:
- PostgreSQL: 10Gi
- Redis: 1Gi  
- MinIO: 20Gi
- RIB Data: 2Gi

Adjust storage sizes in `k8s/storage.yaml` based on your needs.

### Resource Limits

Default resource requests and limits are configured for each component. Adjust based on your cluster capacity and expected load.

## Monitoring

### Health Checks

All services include:
- **Liveness probes**: Detect when containers need to be restarted
- **Readiness probes**: Determine when containers are ready to receive traffic

### Scaling

HPA is configured to scale based on:
- **CPU utilization**: Target 70% average
- **Memory utilization**: Target 80% average (backend only)

## Security

### Network Policies

Consider adding NetworkPolicies to restrict inter-pod communication:

```yaml
# Example: Only allow backend to access database
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: postgres-access
  namespace: rib
spec:
  podSelector:
    matchLabels:
      component: postgres
  policyTypes:
  - Ingress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          component: backend
```

### TLS/HTTPS

To enable HTTPS:

1. Obtain a TLS certificate for your domain
2. Create a TLS secret:
   ```bash
   kubectl create secret tls rib-tls-secret \
     --cert=path/to/cert.pem \
     --key=path/to/key.pem \
     -n rib
   ```
3. Uncomment the TLS section in `k8s/ingress.yaml`

### Secrets Management

For production deployments, consider using:
- **External Secrets Operator** for integration with cloud secret managers
- **Sealed Secrets** for encrypted secrets in Git
- **Kubernetes CSI Secret Store** drivers

## Troubleshooting

### Check pod status
```bash
kubectl get pods -n rib
kubectl describe pod <pod-name> -n rib
kubectl logs <pod-name> -n rib
```

### Check services
```bash
kubectl get services -n rib
kubectl describe service <service-name> -n rib
```

### Check ingress
```bash
kubectl get ingress -n rib
kubectl describe ingress rib-ingress -n rib
```

### Common Issues

1. **ImagePullBackOff**: Ensure images are pushed to registry and accessible
2. **Pending PVCs**: Check if your cluster has a default StorageClass
3. **Ingress not working**: Verify NGINX Ingress Controller is installed
4. **Database connection errors**: Check if PostgreSQL pod is ready and service is accessible

## Production Considerations

### High Availability

For production deployments:

1. **Multiple node deployment**: Ensure pods are distributed across nodes
2. **Database replication**: Consider PostgreSQL streaming replication
3. **Object storage**: Use cloud-based S3 instead of MinIO for better durability
4. **Load balancing**: Use cloud load balancers instead of NodePort services

### Backup Strategy

1. **Database backups**: Implement regular PostgreSQL backups
2. **Object storage**: Configure cross-region replication for MinIO/S3
3. **Configuration backups**: Version control your Kubernetes manifests

### Monitoring and Observability

Consider adding:
- **Prometheus**: Metrics collection
- **Grafana**: Visualization dashboards  
- **Jaeger/Zipkin**: Distributed tracing
- **ELK Stack**: Centralized logging

## Cleanup

To remove the entire deployment:

```bash
kubectl delete namespace rib
```

This will remove all resources in the rib namespace.