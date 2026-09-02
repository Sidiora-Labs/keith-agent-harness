#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$root/scripts/lib/deploy-common.sh"

execute=false
app="${KEITH_FLY_APP:-keith-agent}"
region="${KEITH_FLY_REGION:-iad}"
volume_size="${KEITH_FLY_VOLUME_GB:-20}"
config="$root/deploy/fly/fly.toml"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=true ;;
    --app) app="${2:?missing app}"; shift ;;
    --region) region="${2:?missing region}"; shift ;;
    --volume-size) volume_size="${2:?missing size}"; shift ;;
    -h|--help) echo "usage: $0 [--execute] [--app NAME] [--region REGION] [--volume-size GB]"; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
  shift
done

echo "Fly.io plan"
print_command flyctl apps create "$app"
print_command flyctl volumes create keith_data --app "$app" --region "$region" --size "$volume_size" --yes
echo "  flyctl secrets import --app $app  # secret values are supplied over stdin"
print_command flyctl deploy "$root" --app "$app" --config "$config" --remote-only
if ! require_execute_approval "$execute"; then
  echo "Plan only. No Fly.io resources were changed."
  exit 0
fi

require_tool flyctl
require_tool jq
if [[ -z "${KEITH_PUBLIC_ORIGIN:-}" ]]; then
  export KEITH_PUBLIC_ORIGIN="https://${app}.fly.dev"
fi
require_public_configuration

if ! flyctl status --app "$app" >/dev/null 2>&1; then
  flyctl apps create "$app"
fi
if ! flyctl volumes list --app "$app" --json | grep -q '"name"[[:space:]]*:[[:space:]]*"keith_data"'; then
  flyctl volumes create keith_data --app "$app" --region "$region" --size "$volume_size" --yes
fi

secret_file="$(mktemp /tmp/keith-fly-secrets.XXXXXX)"
chmod 0600 "$secret_file"
cleanup_secret() {
  rm -f "$secret_file"
}
trap cleanup_secret EXIT
printf 'KEITH_WEB_LOGIN_SECRET=%s\nKEITH_PUBLIC_ORIGIN=%s\n' \
  "$KEITH_WEB_LOGIN_SECRET" "$KEITH_PUBLIC_ORIGIN" > "$secret_file"
for variable in KEITH_CREDENTIAL_KEY KEITH_OPENAI_COMPAT_API_KEY KEITH_PLATFORM_API_KEY; do
  [[ -n "${!variable:-}" ]] && printf '%s=%s\n' "$variable" "${!variable}" >> "$secret_file"
done
while IFS= read -r variable; do
  [[ -n "$variable" && "$variable" != "GITHUB_TOKEN" ]] || continue
  [[ -n "${!variable:-}" ]] && printf '%s=%s\n' "$variable" "${!variable}" >> "$secret_file"
done < <(jq -r '.providers[].credential_environment | select(. != "")' "$root/packaging/providers.json" | sort -u)
flyctl secrets import --app "$app" < "$secret_file"
flyctl deploy "$root" --app "$app" --config "$config" --remote-only
