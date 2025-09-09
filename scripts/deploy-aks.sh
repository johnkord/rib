#!/usr/bin/env bash
set -euo pipefail

# Deploy RIB (single backend image serving embedded frontend) to AKS using an existing or newly created Azure Container Registry.
# Requirements:
#  - az CLI logged in (az login)
#  - kubectl context pointing to target AKS (or provide AKS name)
#  - docker daemon available
#  - kustomize plugin is integrated in kubectl (kubectl >=1.24) OR separate 'kustomize' binary (optional)
#
# Usage (minimal):
#   ./scripts/deploy-aks.sh \
#     --resource-group jk-aks-rg-2 \
#     --aks-name <EXISTING_AKS_NAME> \
#     --acr-name ribacr123 \
#     --tag v0.1.0
#
# Idempotent: re-running updates images & reapplies manifests.
#
# Notes:
#  - If you prefer to EMBED the frontend into the Rust binary (single container) you can skip the frontend image.
#  - See README section 'Kubernetes / AKS' (to be added) for more detail.

RESOURCE_GROUP=""
AKS_NAME=""
ACR_NAME=""
TAG="latest"
PUSH_FRONTEND=0
OVERLAY="k8s/overlays/aks"
RIB_IMAGE_NAME="rib"
ENABLE_HTTPS=0
DOMAIN_FQDN=""
ACME_EMAIL=""
LETSENCRYPT_ENV="staging"   # staging | prod
DNS_ZONE=""                 # optional override of zone (apex) if auto-derivation incorrect
SKIP_DNS=0                   # skip DNS zone / A record management (external DNS provider)

while [[ $# -gt 0 ]]; do
  case $1 in
    --resource-group|-g) RESOURCE_GROUP=$2; shift 2;;
    --aks-name) AKS_NAME=$2; shift 2;;
    --acr-name) ACR_NAME=$2; shift 2;;
    --tag) TAG=$2; shift 2;;
  --overlay) OVERLAY=$2; shift 2;;
  --enable-https) ENABLE_HTTPS=1; shift 1;;
  --domain) DOMAIN_FQDN=$2; shift 2;;
  --acme-email) ACME_EMAIL=$2; shift 2;;
  --letsencrypt-env) LETSENCRYPT_ENV=$2; shift 2;;
  --dns-zone) DNS_ZONE=$2; shift 2;;
  --skip-dns) SKIP_DNS=1; shift 1;;
    *) echo "Unknown arg: $1"; exit 1;;
  esac
done

if [[ -z ${RESOURCE_GROUP} || -z ${AKS_NAME} || -z ${ACR_NAME} ]]; then
  echo "Missing required args. See script header for usage." >&2
  exit 1
fi

if [[ ${ENABLE_HTTPS} -eq 1 ]]; then
  if [[ -z ${DOMAIN_FQDN} || -z ${ACME_EMAIL} ]]; then
    echo "--enable-https requires --domain <fqdn> and --acme-email <email>." >&2
    exit 1
  fi
  if [[ ${LETSENCRYPT_ENV} != "staging" && ${LETSENCRYPT_ENV} != "prod" ]]; then
    echo "--letsencrypt-env must be staging or prod" >&2; exit 1;
  fi
fi

if ! command -v az >/dev/null; then
  echo "az CLI not found" >&2; exit 1; fi
if ! command -v docker >/dev/null; then
  echo "docker not found" >&2; exit 1; fi
if ! command -v kubectl >/dev/null; then
  echo "kubectl not found" >&2; exit 1; fi

# Ensure RG exists (will error if not)
az group show -n "${RESOURCE_GROUP}" >/dev/null

echo "[1/8] Ensure / create ACR ${ACR_NAME} in ${RESOURCE_GROUP}" >&2
if ! az acr show -n "${ACR_NAME}" -g "${RESOURCE_GROUP}" >/dev/null 2>&1; then
  az acr create -n "${ACR_NAME}" -g "${RESOURCE_GROUP}" --sku Basic --admin-enabled false
else
  echo "ACR already exists" >&2
fi

ACR_LOGIN_SERVER=$(az acr show -n "${ACR_NAME}" -g "${RESOURCE_GROUP}" --query loginServer -o tsv)

echo "[2/8] ALREADY DONE: Attach ACR to AKS (grants pull permission via managed identity)" >&2
# already done - no need to run again
#az aks update -n "${AKS_NAME}" -g "${RESOURCE_GROUP}" --attach-acr "${ACR_NAME}" >/dev/null

echo "[3/8] Get AKS credentials (merging kubeconfig)" >&2
az aks get-credentials -n "${AKS_NAME}" -g "${RESOURCE_GROUP}" --overwrite-existing >/dev/null

BACKEND_IMAGE="${ACR_LOGIN_SERVER}/${RIB_IMAGE_NAME}:${TAG}"

echo "[4/8] Build backend image (with embedded frontend) ${BACKEND_IMAGE}" >&2
docker build -t "${BACKEND_IMAGE}" .

echo "[5/8] Push image" >&2
docker push "${BACKEND_IMAGE}"

echo "[6/8] Create namespace (if missing)" >&2
kubectl get ns rib >/dev/null 2>&1 || kubectl create namespace rib

# Secrets (only create if absent: allows rotation outside script)
echo "[7/8] Ensure runtime secrets exist (won't overwrite)" >&2
if ! kubectl -n rib get secret rib-secrets >/dev/null 2>&1; then
  echo "Creating rib-secrets placeholder (edit after)" >&2
  kubectl -n rib create secret generic rib-secrets \
    --from-literal=JWT_SECRET="$(openssl rand -hex 24)" \
    --from-literal=DATABASE_URL="postgres://postgres:postgres@postgres:5432/rib" \
    --from-literal=REDIS_URL="redis://redis:6379" \
    --from-literal=S3_ACCESS_KEY="minioadmin" \
    --from-literal=S3_SECRET_KEY="minioadmin" \
    --from-literal=DISCORD_CLIENT_ID="" \
    --from-literal=DISCORD_CLIENT_SECRET=""
else
  echo "Secret rib-secrets already present" >&2
fi

# We dynamically create a temp copy of the overlay preserving its relative depth so '../../base' still resolves.
TMP_DIR=$(mktemp -d)
trap 'rm -rf ${TMP_DIR}' EXIT

# Recreate minimal directory structure expected by overlay: needs ../../base relative to overlay dir
TMP_OVERLAY_DIR="${TMP_DIR}/${OVERLAY}"  # e.g. /tmp/tmp.XYZ/k8s/overlays/aks
TMP_BASE_DIR="${TMP_DIR}/k8s/base"
mkdir -p "${TMP_OVERLAY_DIR}" "${TMP_BASE_DIR}"

# Copy base and overlay contents preserving relative layout
cp -R k8s/base/* "${TMP_BASE_DIR}"/
cp -R "${OVERLAY}"/* "${TMP_OVERLAY_DIR}"/

# Append images override via a sibling kustomization patch (merging is supported by kustomize when multiple kustomizations present)
cat >> "${TMP_OVERLAY_DIR}/kustomization.yaml" <<EOF

# --- auto-injected image override (ACR) ---
images:
  - name: ${ACR_LOGIN_SERVER}/${RIB_IMAGE_NAME}
    newTag: ${TAG}
EOF

echo "[8/8] Apply manifests" >&2
kubectl apply -k "${TMP_OVERLAY_DIR}" >/dev/null

if [[ ${ENABLE_HTTPS} -eq 1 ]]; then
  echo "[HTTPS] Begin automated HTTPS setup for ${DOMAIN_FQDN}" >&2
  # 1. Ensure cert-manager installed (lightweight idempotent check)
  if ! kubectl get ns cert-manager >/dev/null 2>&1; then
    echo "[HTTPS] Installing cert-manager (CRDs + helm)" >&2
    kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.14.4/cert-manager.crds.yaml
    helm repo add jetstack https://charts.jetstack.io >/dev/null
    helm repo update >/dev/null
    helm install cert-manager jetstack/cert-manager \
      --namespace cert-manager \
      --create-namespace \
      --version v1.14.4 >/dev/null
  else
    echo "[HTTPS] cert-manager already present" >&2
  fi

  if [[ ${SKIP_DNS} -eq 0 ]]; then
    # Manage DNS via Azure
    if [[ -z ${DNS_ZONE} ]]; then
      DNS_ZONE=$(echo "${DOMAIN_FQDN}" | awk -F. '{n=NF; if (n<2){print $0}else{print $(n-1)"."$n}}')
      echo "[HTTPS] Derived DNS zone: ${DNS_ZONE}" >&2
    fi
    if ! az network dns zone show -g "${RESOURCE_GROUP}" -n "${DNS_ZONE}" >/dev/null 2>&1; then
      echo "[HTTPS] Creating DNS zone ${DNS_ZONE}" >&2
      az network dns zone create -g "${RESOURCE_GROUP}" -n "${DNS_ZONE}" >/dev/null
      echo "[HTTPS] Delegate registrar NS to Azure for full control." >&2
    else
      echo "[HTTPS] DNS zone ${DNS_ZONE} exists" >&2
    fi
    INGRESS_IP=$(kubectl -n rib get ingress -o jsonpath='{.items[0].status.loadBalancer.ingress[0].ip}') || true
    if [[ -z ${INGRESS_IP} ]]; then
      echo "[HTTPS] Ingress IP pending; re-run later." >&2
      exit 0
    fi
    echo "[HTTPS] Ingress IP: ${INGRESS_IP}" >&2
    HOST_LABEL=$(echo "${DOMAIN_FQDN}" | sed "s/.${DNS_ZONE}//")
    if [[ "${HOST_LABEL}" == "${DOMAIN_FQDN}" ]]; then HOST_LABEL="@"; fi
    if [[ "${HOST_LABEL}" == "@" ]]; then RS_NAME='@'; else RS_NAME="${HOST_LABEL}"; fi
    echo "[HTTPS] Upserting A record ${RS_NAME}.${DNS_ZONE} -> ${INGRESS_IP}" >&2
    if ! az network dns record-set a show -g "${RESOURCE_GROUP}" -z "${DNS_ZONE}" -n "${RS_NAME}" >/dev/null 2>&1; then
      az network dns record-set a add-record -g "${RESOURCE_GROUP}" -z "${DNS_ZONE}" -n "${RS_NAME}" -a "${INGRESS_IP}" >/dev/null
    else
      az network dns record-set a update -g "${RESOURCE_GROUP}" -z "${DNS_ZONE}" -n "${RS_NAME}" --set aRecords=[] >/dev/null
      az network dns record-set a add-record -g "${RESOURCE_GROUP}" -z "${DNS_ZONE}" -n "${RS_NAME}" -a "${INGRESS_IP}" >/dev/null
    fi
  else
    echo "[HTTPS] SKIP_DNS=1: Assuming external DNS already points ${DOMAIN_FQDN} to ingress IP" >&2
  fi

  # 5. Apply ClusterIssuers (templated email)
  ISSUER_FILE=$(mktemp)
  sed "s/you@example.com/${ACME_EMAIL}/" k8s/cert-manager/cluster-issuers.yaml > "${ISSUER_FILE}"
  kubectl apply -f "${ISSUER_FILE}" >/dev/null

  # 6. Patch ingress with host + issuer & TLS secret (reuse existing or create new)
  ISSUER_NAME="letsencrypt-${LETSENCRYPT_ENV}"
  TLS_SECRET="rib-tls"
  echo "[HTTPS] Patching ingress with host=${DOMAIN_FQDN} issuer=${ISSUER_NAME}" >&2
  kubectl -n rib patch ingress $(kubectl -n rib get ingress -o jsonpath='{.items[0].metadata.name}') \
    --type='json' -p="[
      {\"op\":\"replace\",\"path\":\"/spec/rules/0/host\",\"value\":\"${DOMAIN_FQDN}\"},
      {\"op\":\"replace\",\"path\":\"/spec/tls/0/hosts/0\",\"value\":\"${DOMAIN_FQDN}\"}
    ]" >/dev/null || echo "[HTTPS] Warning: could not patch host (ensure ingress has expected structure)" >&2
  kubectl -n rib annotate ingress $(kubectl -n rib get ingress -o jsonpath='{.items[0].metadata.name}') \
    cert-manager.io/cluster-issuer="${ISSUER_NAME}" --overwrite >/dev/null

  echo "[HTTPS] Waiting for Certificate request (non-blocking). Check status with:" >&2
  echo "       kubectl -n rib get certificate" >&2
  echo "[HTTPS] If using staging, switch to prod after success: rerun with --letsencrypt-env prod" >&2
fi

echo "Deployment submitted. Status:" >&2
kubectl -n rib get deploy,sts,svc | grep -E 'rib-|postgres|redis|minio'

echo "Next steps:" >&2
echo "  - Wait for ingress external IP: kubectl -n rib get ingress"

echo "Done." >&2
