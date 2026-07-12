#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || -z "$1" || "$1" =~ [[:space:]] || "$1" == *'#'* ]]; then
  echo "usage: $0 <registry/repository:tag>" >&2
  exit 2
fi

image="$1"
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
manifest=$(mktemp)
trap 'rm -f "$manifest"' EXIT

kubectl kustomize "$script_dir/../k8s/overlays/aks" > "$manifest"
image_lines=$(grep -Ec '^[[:space:]]+image: .*/rib:[^[:space:]]+$' "$manifest")
if [[ "$image_lines" -ne 1 ]]; then
  echo "expected exactly one RIB image, found $image_lines" >&2
  exit 1
fi

sed -Ei "s#^([[:space:]]+image: ).*/rib:[^[:space:]]+\$#\1${image}#" "$manifest"
if [[ $(grep -Fc "image: $image" "$manifest") -ne 1 ]]; then
  echo "failed to set RIB image to $image" >&2
  exit 1
fi

cat "$manifest"
