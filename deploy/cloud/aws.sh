#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
cluster="${KEITH_AWS_CLUSTER:-keith}"
region="${KEITH_AWS_REGION:-us-east-1}"
size="${KEITH_AWS_NODE_TYPE:-m6i.xlarge}"
nodes="${KEITH_AWS_NODE_COUNT:-1}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    --cluster) cluster="${2:?missing cluster}"; shift ;;
    --region) region="${2:?missing region}"; shift ;;
    --size) size="${2:?missing instance type}"; shift ;;
    --nodes) nodes="${2:?missing node count}"; shift ;;
    -h|--help) echo "usage: $0 [--execute] [--cluster NAME] [--region REGION] [--size INSTANCE] [--nodes N]"; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

echo "Amazon EKS plan"
print_command eksctl create cluster --name "$cluster" --region "$region" --node-type "$size" --nodes "$nodes" --managed
print_command aws eks update-kubeconfig --name "$cluster" --region "$region"
print_command "$root/deploy/kubernetes/deploy.sh" --execute
if ! require_execute_approval "$execute"; then
  echo "Plan only. No AWS resources were changed."
  exit 0
fi

require_tool aws
require_tool eksctl
require_public_configuration
if ! eksctl get cluster --name "$cluster" --region "$region" >/dev/null 2>&1; then
  eksctl create cluster --name "$cluster" --region "$region" --node-type "$size" --nodes "$nodes" --managed
fi
aws eks update-kubeconfig --name "$cluster" --region "$region"
"$root/deploy/kubernetes/deploy.sh" --execute

