# Kubernetes Manifests for RIB

This directory contains a Kustomize-based configuration to deploy the RIB stack (backend API, frontend SPA, PostgreSQL, Redis, MinIO) into a Kubernetes cluster.

## Structure
```
base/          # Core manifests (no secrets, generic images)
overlays/
  dev/         # Development overrides (single replica, dev tags)
  prod/        # Production overrides (versioned tags, higher replicas)
```

## Components
| Component | Kind | Notes |
|-----------|------|-------|
| rib | Deployment + Service | Actix-web API + migrations (runs at startup) |
| postgres | StatefulSet + Service | Single instance (extend with HA if needed) |
| redis | Deployment + Service | Optional cache / rate limit store |
| minio | StatefulSet + Service | S3-compatible object storage |
| ingress | Ingress | Fronts both frontend and backend |
| config | ConfigMap | Non-sensitive configuration |
| secrets | Secret | Sensitive values (EXCLUDED from git) |

## Quick Start (Dev)
```bash
# Create secrets (example values!)
kubectl create namespace rib
kubectl -n rib create secret generic rib-secrets \
  --from-literal=JWT_SECRET="$(openssl rand -hex 24)" \
  --from-literal=DATABASE_URL="postgres://postgres:postgres@postgres:5432/rib" \
  --from-literal=REDIS_URL="redis://redis:6379" \
  --from-literal=S3_ACCESS_KEY="minioadmin" \
  --from-literal=S3_SECRET_KEY="minioadmin" \
  --from-literal=DISCORD_CLIENT_ID="" \
  --from-literal=DISCORD_CLIENT_SECRET=""

# Deploy
kubectl apply -k k8s/overlays/dev
```

Then visit http://localhost:8081 . API under /api and docs under /docs.

## Production Notes
- Replace image names in overlays/prod with your published tags (CI should push <ACR>.azurecr.io/rib:<tag>)
- Provide a TLS secret `rib-tls` (e.g. cert-manager) for the Ingress.
- Consider externalizing Postgres (managed service) and MinIO (S3) for durability.
- Set `ENABLE_HSTS=true` via ConfigMap or override patch after TLS is confirmed working.
- Add resource requests/limits tuning after observing baseline metrics.
- Enable autoscaling (HPA already defined) with metrics-server installed.
- Configure backups for PostgreSQL PersistentVolume.

## Customization
Use additional patches in an overlay:
```yaml
patches:
  - target:
      kind: ConfigMap
      name: rib-config
    patch: |-
      - op: add
        path: /data/RUST_LOG
        value: debug
```

## Future Enhancements
- Separate read/write Postgres services for replicas
- Add PodDisruptionBudgets
- Add NetworkPolicies
- Add ServiceMonitor (Prometheus) once metrics endpoint available
- Add initContainer for explicit migration step if startup time grows

## Cleanup
```bash
kubectl delete -k k8s/overlays/dev
```

---
This configuration is a starting point; harden before internet exposure (RBAC, NetworkPolicy, backups, scanning).
