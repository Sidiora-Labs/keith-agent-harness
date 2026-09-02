#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
cluster="${KEITH_GCP_CLUSTER:-keith}"
region="${KEITH_GCP_REGION:-us-central1}"
project="${KEITH_GCP_PROJECT:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    --cluster) cluster="${2:?missing cluster}"; shift ;;
    --region) region="${2:?missing region}"; shift ;;
    --project) project="${2:?missing project}"; shift ;;
    -h|--help) echo "usage: $0 [--execute] --project PROJECT [--cluster NAME] [--region REGION]"; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done
[[ -n "$project" ]] || die "--project or KEITH_GCP_PROJECT is required"

echo "Google Kubernetes Engine Autopilot plan"
print_command gcloud container clusters create-auto "$cluster" --project "$project" --region "$region"
print_command gcloud container clusters get-credentials "$cluster" --project "$project" --region "$region"
print_command "$root/deploy/kubernetes/deploy.sh" --execute
if ! require_execute_approval "$execute"; then
  echo "Plan only. No Google Cloud resources were changed."
  exit 0
fi

require_tool gcloud
require_public_configuration
if ! gcloud container clusters describe "$cluster" --project "$project" --region "$region" >/dev/null 2>&1; then
  gcloud container clusters create-auto "$cluster" --project "$project" --region "$region"
fi
gcloud container clusters get-credentials "$cluster" --project "$project" --region "$region"
"$root/deploy/kubernetes/deploy.sh" --execute

