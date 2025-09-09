#!/usr/bin/env bash
# Idempotently grant the GitHub Actions workload identity service principal
# permission to read secrets from the production Key Vault.
#
# Uses Azure RBAC (NOT legacy access policies). Assigns the "Key Vault Secrets User" role
# at the Key Vault scope. If already assigned, it exits cleanly. Optionally you can
# specify a different role via --role (e.g. Key Vault Administrator) but downgrade
# to the least-privilege role after verification.
#
# Requirements:
#  - Azure CLI logged in (az login) with rights to assign roles (Owner or User Access Administrator)
#  - AZURE_CLIENT_ID exported (the appId / client ID of the GitHub OIDC service principal)
#  - Subscription contains Key Vault `rib-prod-kv` (override with --kv-name)
#
# Usage:
#   AZURE_CLIENT_ID=<app-id> ./scripts/setup-kv-rbac.sh
#   AZURE_CLIENT_ID=<app-id> ./scripts/setup-kv-rbac.sh --role "Key Vault Administrator"
#   AZURE_CLIENT_ID=<app-id> ./scripts/setup-kv-rbac.sh --kv-name rib-prod-kv --subscription bc9cb398-8a5e-4c5d-9ddc-62aafed201c0
#
# After running, re-run the GitHub deploy workflow. Propagation may take 1-3 minutes.

set -euo pipefail

ROLE="Key Vault Secrets User"
KV_NAME="rib-prod-kv"
SUBSCRIPTION_ID="bc9cb398-8a5e-4c5d-9ddc-62aafed201c0"
WAIT_SECONDS=10
MAX_RETRIES=12 # ~2 minutes

while [[ $# -gt 0 ]]; do
  case "$1" in
    --role)
      ROLE="$2"; shift 2;;
    --kv-name)
      KV_NAME="$2"; shift 2;;
    --subscription|--subscription-id)
      SUBSCRIPTION_ID="$2"; shift 2;;
    --help|-h)
      grep '^# ' "$0" | sed 's/^# //'; exit 0;;
    *)
      echo "Unknown arg: $1" >&2; exit 1;;
  esac
done

if [[ -z "${AZURE_CLIENT_ID:-}" ]]; then
  echo "AZURE_CLIENT_ID env var required (the service principal client ID used by GitHub Actions)." >&2
  exit 1
fi

echo "[INFO] Subscription:    $SUBSCRIPTION_ID"
echo "[INFO] Key Vault name:  $KV_NAME"
echo "[INFO] Target role:     $ROLE"
echo "[INFO] SP (client id):  $AZURE_CLIENT_ID"

echo "[STEP] Resolving service principal object id..."
SP_OBJECT_ID=$(az ad sp show --id "$AZURE_CLIENT_ID" --query id -o tsv 2>/dev/null || true)
if [[ -z "$SP_OBJECT_ID" ]]; then
  echo "[ERROR] Could not resolve service principal by AZURE_CLIENT_ID=$AZURE_CLIENT_ID" >&2
  exit 1
fi

echo "[INFO] SP object id:    $SP_OBJECT_ID"

echo "[STEP] Fetching Key Vault ID..."
KV_ID=$(az keyvault show -n "$KV_NAME" --subscription "$SUBSCRIPTION_ID" --query id -o tsv 2>/dev/null || true)
if [[ -z "$KV_ID" ]]; then
  echo "[ERROR] Key Vault '$KV_NAME' not found in subscription $SUBSCRIPTION_ID" >&2
  exit 1
fi

echo "[INFO] Key Vault ID:     $KV_ID"

echo "[STEP] Checking existing role assignment..."
ASSIGN_COUNT=$(az role assignment list --assignee "$SP_OBJECT_ID" --scope "$KV_ID" --query "[?roleDefinitionName=='$ROLE'] | length(@)" -o tsv)
if [[ "$ASSIGN_COUNT" -gt 0 ]]; then
  echo "[OK] Role '$ROLE' already assigned to SP at Key Vault scope."
else
  echo "[STEP] Creating role assignment '$ROLE' ..."
  az role assignment create --assignee-object-id "$SP_OBJECT_ID" --assignee-principal-type ServicePrincipal --role "$ROLE" --scope "$KV_ID" >/dev/null
  echo "[OK] Role assignment created."
fi

echo "[STEP] Verifying access (attempting secret list)..."
RETRY=0
until az keyvault secret list --vault-name "$KV_NAME" --subscription "$SUBSCRIPTION_ID" --maxresults 1 >/dev/null 2>&1; do
  (( RETRY++ )) || true
  if (( RETRY > MAX_RETRIES )); then
    echo "[WARN] Still cannot list secrets after $((WAIT_SECONDS*MAX_RETRIES))s. RBAC propagation may be delayed." >&2
    echo "       Re-run the GitHub workflow later. You can manually test with:"
    echo "       az keyvault secret list --vault-name $KV_NAME --subscription $SUBSCRIPTION_ID --maxresults 1"
    exit 0
  fi
  echo "[INFO] Waiting for RBAC propagation... (${RETRY}/${MAX_RETRIES})"
  sleep "$WAIT_SECONDS"
done

echo "[SUCCESS] Service principal now has secret read access to Key Vault '$KV_NAME'."
