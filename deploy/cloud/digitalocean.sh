#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
cluster="${KEITH_DO_CLUSTER:-keith}"
region="${KEITH_DO_REGION:-nyc1}"
size="${KEITH_DO_NODE_SIZE:-s-4vcpu-8gb}"
nodes="${KEITH_DO_NODE_COUNT:-1}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    --cluster) cluster="${2:?missing cluster}"; shift ;;
    --region) region="${2:?missing region}"; shift ;;
    --size) size="${2:?missing size}"; shift ;;
    --nodes) nodes="${2:?missing node count}"; shift ;;
    -h|--help) echo "usage: $0 [--execute] [--cluster NAME] [--region REGION] [--size SLUG] [--nodes N]"; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

echo "DigitalOcean Kubernetes plan"
print_command doctl kubernetes cluster create "$cluster" --region "$region" --size "$size" --count "$nodes" --wait
print_command doctl kubernetes cluster kubeconfig save "$cluster"
print_command "$root/deploy/kubernetes/deploy.sh" --execute
if ! require_execute_approval "$execute"; then
  echo "Plan only. No DigitalOcean resources were changed."
  exit 0
fi

require_tool doctl
require_public_configuration
if ! doctl kubernetes cluster get "$cluster" >/dev/null 2>&1; then
  doctl kubernetes cluster create "$cluster" --region "$region" --size "$size" --count "$nodes" --wait
fi
doctl kubernetes cluster kubeconfig save "$cluster"
"$root/deploy/kubernetes/deploy.sh" --execute

