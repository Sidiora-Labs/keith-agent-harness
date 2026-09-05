#!/usr/bin/env bash
set -Eeuo pipefail

image="${1:-keith-agent:ci}"
port="${2:-17341}"
name="keith-smoke-${GITHUB_RUN_ID:-local}-${RANDOM}"
data_volume="${name}-data"
workspace_volume="${name}-workspace"
export KEITH_WEB_LOGIN_SECRET="${KEITH_WEB_LOGIN_SECRET:-ci-login-secret-with-more-than-32-bytes}"
export KEITH_OPENAI_COMPAT_API_KEY="${KEITH_OPENAI_COMPAT_API_KEY:-ci-openai-compat-secret-more-than-32-bytes}"
export OPENAI_API_KEY="${OPENAI_API_KEY:-ci-provider-credential-used-only-for-bootstrap}"
origin="http://127.0.0.1:${port}"
cookie_file="$(mktemp /tmp/keith-smoke-cookie.XXXXXX)"
auth_file="$(mktemp /tmp/keith-smoke-auth.XXXXXX)"
chmod 0600 "$cookie_file" "$auth_file"

cleanup() {
  status=$?
  trap - EXIT
  if [[ "$status" -ne 0 ]]; then
    docker logs "$name" 2>/dev/null || true
  fi
  docker rm --force "$name" >/dev/null 2>&1 || true
  docker volume rm --force "$data_volume" "$workspace_volume" >/dev/null 2>&1 || true
  rm -f "$cookie_file" "$auth_file"
  exit "$status"
}
trap cleanup EXIT

docker volume create "$data_volume" >/dev/null
docker volume create "$workspace_volume" >/dev/null
docker run --detach --name "$name" \
  --publish "127.0.0.1:${port}:7341" \
  --volume "${data_volume}:/var/lib/keith" \
  --volume "${workspace_volume}:/workspace" \
  --env "KEITH_PUBLIC_ORIGIN=$origin" \
  --env KEITH_WEB_LOGIN_SECRET \
  --env KEITH_OPENAI_COMPAT_API_KEY \
  --env OPENAI_API_KEY \
  "$image" >/dev/null

for _ in $(seq 1 90); do
  state="$(docker inspect --format '{{.State.Status}}' "$name")"
  health="$(docker inspect --format '{{.State.Health.Status}}' "$name")"
  [[ "$health" == "healthy" ]] && break
  [[ "$state" == "running" ]] || break
  sleep 2
done
[[ "$(docker inspect --format '{{.State.Status}}' "$name")" == "running" ]]
[[ "$(docker inspect --format '{{.State.Health.Status}}' "$name")" == "healthy" ]]
docker exec "$name" sh -lc '
  test "$(stat -c %u:%g /var/lib/keith)" = "10001:10001"
  test "$(stat -c %u:%g /workspace)" = "10001:10001"
  found=0
  for command_path in /proc/[0-9]*/comm; do
    read -r command_name < "$command_path" || continue
    case "$command_name" in
      agentd|agent-web)
        status_path="${command_path%/comm}/status"
        test "$(grep "^Uid:" "$status_path" | cut -f2)" = "10001"
        found=$((found + 1))
        ;;
    esac
  done
  test "$found" -ge 2
'

curl --fail --silent --show-error "$origin/login" >/dev/null
curl --fail --silent --show-error \
  --cookie-jar "$cookie_file" \
  --header "Origin: $origin" \
  --data-urlencode "password=$KEITH_WEB_LOGIN_SECRET" \
  "$origin/auth/session" >/dev/null
curl --fail --silent --show-error \
  --cookie "$cookie_file" \
  "$origin/api/bootstrap" | jq -e '.profiles' >/dev/null

printf 'Authorization: Bearer %s\n' "$KEITH_OPENAI_COMPAT_API_KEY" > "$auth_file"
curl --fail --silent --show-error \
  --header @"$auth_file" \
  "$origin/v1/models" | jq -e '.object == "list"' >/dev/null

docker restart --timeout 20 "$name" >/dev/null
for _ in $(seq 1 90); do
  state="$(docker inspect --format '{{.State.Status}}' "$name")"
  health="$(docker inspect --format '{{.State.Health.Status}}' "$name")"
  [[ "$health" == "healthy" ]] && break
  [[ "$state" == "running" ]] || break
  sleep 2
done
[[ "$(docker inspect --format '{{.State.Health.Status}}' "$name")" == "healthy" ]]
docker exec "$name" test -s /var/lib/keith/state.sqlite
rm -f "$cookie_file"
curl --fail --silent --show-error \
  --cookie-jar "$cookie_file" \
  --header "Origin: $origin" \
  --data-urlencode "password=$KEITH_WEB_LOGIN_SECRET" \
  "$origin/auth/session" >/dev/null
curl --fail --silent --show-error \
  --cookie "$cookie_file" \
  "$origin/api/bootstrap" | jq -e '.profiles' >/dev/null

echo "Keith container smoke journey passed: non-root startup, durable restart, login, bootstrap, and OpenAI model discovery"
