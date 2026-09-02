#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
render=false
namespace="${KEITH_KUBERNETES_NAMESPACE:-keith}"
release="${KEITH_HELM_RELEASE:-keith}"
image="${KEITH_IMAGE:-ghcr.io/machinecity/keith-agent:main}"
chart="$root/deploy/kubernetes/helm/keith"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    --render) render=true ;;
    --namespace) namespace="${2:?missing namespace}"; shift ;;
    --release) release="${2:?missing release}"; shift ;;
    --image) image="${2:?missing image}"; shift ;;
    -h|--help)
      echo "usage: $0 [--render] [--execute] [--namespace NAME] [--release NAME] [--image IMAGE:TAG]"
      exit 0
      ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

require_tool helm
repository="${image%:*}"
tag="${image##*:}"
[[ "$repository" != "$tag" ]] || die "image must include an explicit tag"
origin="${KEITH_PUBLIC_ORIGIN:-http://localhost:7341}"

helm lint "$chart" \
  --set-string "image.repository=$repository" \
  --set-string "image.tag=$tag" \
  --set-string "config.publicOrigin=$origin" >/dev/null

if [[ "$render" == "true" ]]; then
  helm template "$release" "$chart" \
    --namespace "$namespace" \
    --set-string "image.repository=$repository" \
    --set-string "image.tag=$tag" \
    --set-string "config.publicOrigin=$origin"
fi

echo "Keith Kubernetes plan"
echo "  context:   $(kubectl config current-context 2>/dev/null || echo '<not selected>')"
echo "  namespace: $namespace"
echo "  release:   $release"
echo "  image:     $image"
echo "  origin:    $origin"
echo "  state:     one StatefulSet replica with a 20Gi ReadWriteOnce PVC"

if ! require_execute_approval "$execute"; then
  echo "Plan only. Re-run with --execute and KEITH_DEPLOY_APPROVED=YES."
  exit 0
fi

require_tool kubectl
require_tool jq
require_public_configuration

kubectl create namespace "$namespace" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
secret_file="$(mktemp /tmp/keith-kubernetes-secret.XXXXXX)"
chmod 0600 "$secret_file"
cleanup_secret() {
  rm -f "$secret_file"
}
trap cleanup_secret EXIT

printf 'KEITH_WEB_LOGIN_SECRET=%s\n' "$KEITH_WEB_LOGIN_SECRET" > "$secret_file"
for variable in KEITH_CREDENTIAL_KEY KEITH_OPENAI_COMPAT_API_KEY KEITH_PLATFORM_API_KEY; do
  if [[ -n "${!variable:-}" ]]; then
    printf '%s=%s\n' "$variable" "${!variable}" >> "$secret_file"
  fi
done
if [[ -f "$root/packaging/providers.json" ]] && command -v jq >/dev/null 2>&1; then
  while IFS= read -r variable; do
    [[ -n "$variable" && "$variable" != "GITHUB_TOKEN" ]] || continue
    if [[ -n "${!variable:-}" ]]; then
      [[ "${!variable}" != *$'\n'* ]] || die "$variable must not contain a newline"
      printf '%s=%s\n' "$variable" "${!variable}" >> "$secret_file"
    fi
  done < <(jq -r '.providers[].credential_environment | select(. != "")' "$root/packaging/providers.json" | sort -u)
fi

kubectl create secret generic keith-secrets \
  --namespace "$namespace" \
  --from-env-file="$secret_file" \
  --dry-run=client -o yaml | kubectl apply -f - >/dev/null

helm upgrade --install "$release" "$chart" \
  --namespace "$namespace" \
  --atomic --wait --timeout 15m \
  --set-string "image.repository=$repository" \
  --set-string "image.tag=$tag" \
  --set-string "config.publicOrigin=$KEITH_PUBLIC_ORIGIN"
