#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    -h|--help) echo "usage: $0 [--execute]"; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

require_tool railway
cd "$root"
echo "Railway will evaluate .railway/railway.ts and redact variable values."
railway config plan

if ! require_execute_approval "$execute"; then
  echo "Plan only. No Railway resources were changed."
  exit 0
fi

require_public_configuration
echo "Applying the reviewed Railway plan interactively."
railway config apply

