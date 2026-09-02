#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
cluster="${KEITH_AZURE_CLUSTER:-keith}"
group="${KEITH_AZURE_RESOURCE_GROUP:-keith}"
location="${KEITH_AZURE_LOCATION:-eastus}"
size="${KEITH_AZURE_NODE_SIZE:-Standard_D4s_v5}"
nodes="${KEITH_AZURE_NODE_COUNT:-1}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    --cluster) cluster="${2:?missing cluster}"; shift ;;
    --resource-group) group="${2:?missing resource group}"; shift ;;
    --location) location="${2:?missing location}"; shift ;;
    --size) size="${2:?missing VM size}"; shift ;;
    --nodes) nodes="${2:?missing node count}"; shift ;;
    -h|--help) echo "usage: $0 [--execute] [--cluster NAME] [--resource-group NAME] [--location REGION] [--size VM] [--nodes N]"; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

echo "Azure Kubernetes Service plan"
print_command az group create --name "$group" --location "$location"
print_command az aks create --resource-group "$group" --name "$cluster" --location "$location" --node-count "$nodes" --node-vm-size "$size" --enable-managed-identity --generate-ssh-keys
print_command az aks get-credentials --resource-group "$group" --name "$cluster" --overwrite-existing
print_command "$root/deploy/kubernetes/deploy.sh" --execute
if ! require_execute_approval "$execute"; then
  echo "Plan only. No Azure resources were changed."
  exit 0
fi

require_tool az
require_public_configuration
az group create --name "$group" --location "$location" --output none
if ! az aks show --resource-group "$group" --name "$cluster" >/dev/null 2>&1; then
  az aks create \
    --resource-group "$group" --name "$cluster" --location "$location" \
    --node-count "$nodes" --node-vm-size "$size" \
    --enable-managed-identity --generate-ssh-keys --output none
fi
az aks get-credentials --resource-group "$group" --name "$cluster" --overwrite-existing --output none
"$root/deploy/kubernetes/deploy.sh" --execute

