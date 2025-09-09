#!/usr/bin/env bash
set -euo pipefail

# Azure Front Door + WAF bootstrap for rib
# Idempotent provisioning of basic AFD + WAF rate limiting (Global + Auth) and optional geo blocking.
#
# Example:
#   ./scripts/afd_bootstrap.sh \
#     --resource-group rib-prod \
#     --location eastus \
#     --origin-host api.example.com
#
# Options (defaults in parentheses):
#   --profile-name NAME        (<rg>-afd)
#   --endpoint-name NAME       (<rg>-endpoint)
#   --waf-policy NAME          (<rg>-waf)
#   --route-name NAME          (default-route)
#   --origin-group NAME        (default-origin-group)
#   --origin-name NAME         (default-origin)
#   --global-threshold N       (100)
#   --global-duration MIN      (1)
#   --auth-threshold N         (5)
#   --auth-duration MIN        (1)
#   --auth-path PATH           (/api/auth/login)
#   --geo-block "CC CC"        (disabled)
#   --skip-wait                (disable short propagation sleep)
#   -h|--help                  Show help

RESOURCE_GROUP=""; LOCATION=""; ORIGIN_HOST=""; ORIGIN_HOST_HEADER=""
PROFILE_NAME=""; ENDPOINT_NAME=""; WAF_POLICY_NAME=""
ROUTE_NAME="default-route"; ORIGIN_GROUP_NAME="default-origin-group"; ORIGIN_NAME="default-origin"
GLOBAL_THRESHOLD=100; GLOBAL_DURATION=1
AUTH_THRESHOLD=5; AUTH_DURATION=1; AUTH_PATH="/api/auth/login"
PROBE_PATH="/healthz"
GEO_BLOCK=""; SKIP_WAIT=false

_color(){ local c=$1; shift; printf "\033[%sm%s\033[0m\n" "$c" "$*"; }
info(){ _color 36 "[INFO] $*"; }
warn(){ _color 33 "[WARN] $*"; }
err(){ _color 31 "[ERR ] $*" >&2; }

usage(){ grep -E '^# ' "$0" | sed 's/^# //'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --resource-group) RESOURCE_GROUP="$2"; shift 2;;
    --location) LOCATION="$2"; shift 2;;
  --origin-host) ORIGIN_HOST="$2"; shift 2;;
  --origin-host-header) ORIGIN_HOST_HEADER="$2"; shift 2;;
  --probe-path) PROBE_PATH="$2"; shift 2;;
    --profile-name) PROFILE_NAME="$2"; shift 2;;
    --endpoint-name) ENDPOINT_NAME="$2"; shift 2;;
    --waf-policy) WAF_POLICY_NAME="$2"; shift 2;;
    --route-name) ROUTE_NAME="$2"; shift 2;;
    --origin-group) ORIGIN_GROUP_NAME="$2"; shift 2;;
    --origin-name) ORIGIN_NAME="$2"; shift 2;;
    --global-threshold) GLOBAL_THRESHOLD="$2"; shift 2;;
    --global-duration) GLOBAL_DURATION="$2"; shift 2;;
    --auth-threshold) AUTH_THRESHOLD="$2"; shift 2;;
    --auth-duration) AUTH_DURATION="$2"; shift 2;;
    --auth-path) AUTH_PATH="$2"; shift 2;;
    --geo-block) GEO_BLOCK="$2"; shift 2;;
    --skip-wait) SKIP_WAIT=true; shift;;
    -h|--help) usage; exit 0;;
    *) err "Unknown arg: $1"; usage; exit 1;;
  esac
done

[[ -z $RESOURCE_GROUP || -z $LOCATION || -z $ORIGIN_HOST ]] && { err "--resource-group --location --origin-host required"; exit 1; }

: "${PROFILE_NAME:=${RESOURCE_GROUP}-afd}"; : "${ENDPOINT_NAME:=${RESOURCE_GROUP}-endpoint}"; : "${WAF_POLICY_NAME:=${RESOURCE_GROUP}-waf}"

command -v az >/dev/null || { err "az CLI not found"; exit 1; }
REQUIRED_VER=2.67.0
AZ_VER=$(az version --output json | grep -o '"azure-cli": "[0-9.]*"' | cut -d'"' -f4 || echo 0.0.0)
verlte(){ [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$1" ]; }
if ! verlte "$REQUIRED_VER" "$AZ_VER"; then warn "azure-cli $AZ_VER < $REQUIRED_VER"; fi

exists(){ az "$1" show "${@:2}" >/dev/null 2>&1; }

info "Ensuring resource group $RESOURCE_GROUP ($LOCATION)"; az group create -n "$RESOURCE_GROUP" -l "$LOCATION" 1>/dev/null

if ! exists afd profile -g "$RESOURCE_GROUP" -n "$PROFILE_NAME"; then info "Create profile"; az afd profile create -g "$RESOURCE_GROUP" -n "$PROFILE_NAME" --sku Standard_AzureFrontDoor >/dev/null; else info "Profile exists"; fi
if ! exists afd endpoint -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ENDPOINT_NAME"; then info "Create endpoint"; az afd endpoint create -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ENDPOINT_NAME" --enabled-state Enabled >/dev/null; else info "Endpoint exists"; fi
if ! exists afd origin-group -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ORIGIN_GROUP_NAME"; then info "Create origin-group"; az afd origin-group create -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ORIGIN_GROUP_NAME" --probe-request-type GET --probe-path "$PROBE_PATH" --probe-protocol Https --probe-interval-in-seconds 60 --sample-size 4 --successful-samples-required 3 >/dev/null; else info "Origin-group exists"; fi
if ! exists afd origin -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" --origin-group-name "$ORIGIN_GROUP_NAME" -n "$ORIGIN_NAME"; then info "Create origin"; \
  ORIGIN_CREATE_CMD=(az afd origin create -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" --origin-group-name "$ORIGIN_GROUP_NAME" -n "$ORIGIN_NAME" --host-name "$ORIGIN_HOST" --https-port 443 --priority 1 --weight 1000); \
  if [[ -n $ORIGIN_HOST_HEADER ]]; then ORIGIN_CREATE_CMD+=(--origin-host-header "$ORIGIN_HOST_HEADER"); fi; \
  "${ORIGIN_CREATE_CMD[@]}" >/dev/null; else info "Origin exists"; fi
if ! exists afd route -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" --endpoint-name "$ENDPOINT_NAME" -n "$ROUTE_NAME"; then info "Create route"; az afd route create -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" --endpoint-name "$ENDPOINT_NAME" -n "$ROUTE_NAME" --origin-group "$ORIGIN_GROUP_NAME" --patterns "/*" --https-redirect Enabled --forwarding-protocol MatchRequest --link-to-default-domain Enabled >/dev/null; else info "Route exists"; fi

if ! az network front-door waf-policy show -g "$RESOURCE_GROUP" -n "$WAF_POLICY_NAME" >/dev/null 2>&1; then info "Create WAF policy"; az network front-door waf-policy create -g "$RESOURCE_GROUP" -n "$WAF_POLICY_NAME" --mode Prevention >/dev/null; else info "WAF policy exists"; fi

ensure_rate(){ local name=$1 pri=$2 thr=$3 dur=$4 extra=$5; if az network front-door waf-policy rule show -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --name "$name" >/dev/null 2>&1; then info "Rule $name exists"; else info "Create rule $name"; az network front-door waf-policy rule create -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --name "$name" --priority "$pri" --rule-type RateLimitRule --rate-limit-threshold "$thr" --rate-limit-duration "$dur" --action Block $extra >/dev/null; fi; }
ensure_rate GlobalRateLimit 1 "$GLOBAL_THRESHOLD" "$GLOBAL_DURATION" ""
if az network front-door waf-policy rule show -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --name AuthRateLimit >/dev/null 2>&1; then info "AuthRateLimit exists"; else
  az network front-door waf-policy rule create -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --name AuthRateLimit --priority 2 --rule-type RateLimitRule --rate-limit-threshold "$AUTH_THRESHOLD" --rate-limit-duration "$AUTH_DURATION" --action Block --defer >/dev/null
  az network front-door waf-policy rule match-condition add -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --rule-name AuthRateLimit --match-variables RequestUri --operator Contains --values "$AUTH_PATH" >/dev/null
fi
if [[ -n $GEO_BLOCK ]]; then
  if az network front-door waf-policy rule show -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --name GeoBlock >/dev/null 2>&1; then info "GeoBlock exists"; else
    az network front-door waf-policy rule create -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --name GeoBlock --priority 3 --rule-type MatchRule --action Block --defer >/dev/null
    az network front-door waf-policy rule match-condition add -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" --rule-name GeoBlock --match-variables GeoMatch --operator GeoMatch --values $GEO_BLOCK >/dev/null
  fi
fi

SEC_POLICY=default-security
if az afd security-policy show -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$SEC_POLICY" >/dev/null 2>&1; then info "Security policy exists"; else
  WAF_ID=$(az network front-door waf-policy show -g "$RESOURCE_GROUP" -n "$WAF_POLICY_NAME" --query id -o tsv)
  EP_DOMAIN=$(az afd endpoint show -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ENDPOINT_NAME" --query defaultDomain -o tsv)
  az afd security-policy create -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$SEC_POLICY" --domains "$EP_DOMAIN" --waf-policy "$WAF_ID" >/dev/null
fi

if ! $SKIP_WAIT; then info "Sleep 8s for propagation"; sleep 8 || true; fi

info "Summary"; az afd endpoint show -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ENDPOINT_NAME" --query '{name:name,domain:defaultDomain,state:enabledState}' -o jsonc
az network front-door waf-policy rule list -g "$RESOURCE_GROUP" --policy-name "$WAF_POLICY_NAME" -o table | sed 's/^/[RULE]/'
info "Test: curl -I https://$(az afd endpoint show -g "$RESOURCE_GROUP" --profile-name "$PROFILE_NAME" -n "$ENDPOINT_NAME" --query defaultDomain -o tsv)/health"
exit 0
