#!/usr/bin/env bash

die() {
  echo "$*" >&2
  exit 1
}

require_tool() {
  command -v "$1" >/dev/null 2>&1 || die "Missing required tool: $1"
}

print_command() {
  printf '  '
  printf '%q ' "$@"
  printf '\n'
}

require_execute_approval() {
  local execute="$1"
  if [[ "$execute" != "true" ]]; then
    return 1
  fi
  [[ "${KEITH_DEPLOY_APPROVED:-}" == "YES" ]] || die \
    "--execute requires KEITH_DEPLOY_APPROVED=YES after reviewing the printed plan"
  return 0
}

require_public_configuration() {
  [[ -n "${KEITH_PUBLIC_ORIGIN:-}" ]] || die "KEITH_PUBLIC_ORIGIN is required for deployment"
  [[ "$KEITH_PUBLIC_ORIGIN" =~ ^https?://[^/]+$ ]] || die \
    "KEITH_PUBLIC_ORIGIN must be an http(s) origin without a path"
  [[ -n "${KEITH_WEB_LOGIN_SECRET:-}" ]] || die "KEITH_WEB_LOGIN_SECRET is required for deployment"
  [[ "$KEITH_WEB_LOGIN_SECRET" != *$'\n'* ]] || die "KEITH_WEB_LOGIN_SECRET must not contain a newline"
}

