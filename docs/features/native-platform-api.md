# Native platform API

The native platform API is a separately authenticated HTTP projection of
Keith's `AgentConnection` protocol for trusted server-side integrations. It
preserves native commands, results, events, snapshots, terminal states,
features, profile IDs, and session IDs instead of reducing Keith to a text-only
chat API.

This interface is served by `agent-web` at `/platform/v1`. It is distinct from
the browser routes and the OpenAI-compatible `/v1` adapter, with its own bearer
credential and admission limit.

## Endpoints

| Method and path | Behavior |
| --- | --- |
| `GET /platform/v1/health` | Probe the daemon and count enabled profiles |
| `GET /platform/v1/catalog` | Return native profile and session summaries |
| `GET /platform/v1/capabilities` | Return negotiated daemon feature identifiers |
| `POST /platform/v1/profiles/{profile}/commands` | Execute a scoped native command as JSON or SSE |
| `GET /platform/v1/events/{profile}/{session}` | Subscribe to scoped native events over SSE |

Every route requires the platform bearer. There is no unauthenticated health
route on this interface.

## Enable the API

Set a dedicated random key of at least 32 bytes before starting `agent-web`:

```bash
export KEITH_PLATFORM_API_KEY="$(openssl rand -hex 32)"
export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'

bin/agent-web \
  --bind 127.0.0.1:7341 \
  --origin http://127.0.0.1:7341 \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --asset-root "$PWD/web" \
  --credential-root "$KEITH_DATA_ROOT/credentials" \
  --login-secret-env KEITH_WEB_LOGIN_SECRET
```

The environment name is fixed as `KEITH_PLATFORM_API_KEY`. If it is absent,
all `/platform/v1` routes return `404 platform_api_disabled`. The server refuses
a non-loopback bind while the bridge is enabled unless
`KEITH_PLATFORM_ALLOW_NON_LOOPBACK=true` is also set. That acknowledgement does
not add TLS or network policy.

The default configuration admits 32 concurrent platform catalog, capability,
command, and event operations. Excess work fails with
`429 keith_capacity_exhausted` rather than accumulating an unbounded queue.

## Make requests

Use a protected header file so the bearer is not visible in command arguments:

```bash
platform_headers="$(mktemp)"
chmod 600 "$platform_headers"
printf 'Authorization: Bearer %s\n' \
  "$KEITH_PLATFORM_API_KEY" >"$platform_headers"

curl --fail-with-body \
  --header @"$platform_headers" \
  http://127.0.0.1:7341/platform/v1/health

curl --fail-with-body \
  --header @"$platform_headers" \
  http://127.0.0.1:7341/platform/v1/catalog

curl --fail-with-body \
  --header @"$platform_headers" \
  http://127.0.0.1:7341/platform/v1/capabilities
```

The catalog is the discovery path for profile IDs, workspace IDs, and existing
session IDs. `ListProfiles` is deliberately rejected through the generic
command route.

To list sessions within one profile:

```bash
profile_id='PROFILE_ID_FROM_CATALOG'

curl --fail-with-body \
  --header @"$platform_headers" \
  --header 'Content-Type: application/json' \
  --data "{\
    \"session_id\":null,\
    \"command\":{\
      \"command\":\"list_sessions\",\
      \"parameters\":{\
        \"profile_id\":\"$profile_id\",\
        \"include_archived\":false\
      }\
    }\
  }" \
  "http://127.0.0.1:7341/platform/v1/profiles/$profile_id/commands"
```

A non-streaming command response is the native `CommandResultEnvelope`. Create
a session with `create_session`, then send `submit_prompt` with the same session
ID in both the request's outer `session_id` and the command parameters. Refer to
the generated [AgentConnection protocol schema](../reference/agent-connection.md)
for command and payload shapes rather than hand-maintaining a second schema.

## Command streaming

Add `Accept: text/event-stream` to the command request to receive native
`WireMessage` values as server-sent events:

```bash
curl --fail-with-body --no-buffer \
  --header @"$platform_headers" \
  --header 'Accept: text/event-stream' \
  --header 'Content-Type: application/json' \
  --data @command.json \
  "http://127.0.0.1:7341/platform/v1/profiles/$profile_id/commands"
```

The stream forwards event, snapshot, and terminal messages from the daemon and
ends with a `WireMessage::CommandResult` carrying the matching command ID. It
sends a keep-alive every 15 seconds and disables intermediary transformation.
Consumers must wait for the command result before declaring the operation
accepted or complete.

Command streams are single HTTP operations and do not have a replay cursor. If
the connection ends before the command result, reconcile through the event
subscription and authoritative catalog/session state before retrying the same
logical action.

## Session event subscription and reconnect

Subscribe to an existing session:

```bash
session_id='SESSION_ID_FROM_CATALOG'

curl --fail-with-body --no-buffer \
  --header @"$platform_headers" \
  "http://127.0.0.1:7341/platform/v1/events/$profile_id/$session_id"
```

Each event is a serialized native wire message. Track the newest applied
`generation` and `sequence`, then include them on reconnect:

```text
/platform/v1/events/{profile}/{session}?generation=7&sequence=42
```

The bridge converts that pair into the daemon's native resume cursor for the
session root. The daemon may replay a delta, send a snapshot followed by a
delta, or report an incompatible cursor according to the negotiated protocol.
Clients should apply events in order, ignore duplicates, and replace local
state when an authoritative snapshot arrives.

The event route checks that the session belongs to the URL profile before
opening the stream. A wrong profile/session pair fails with `403 scope_denied`.

Remove the temporary header file after the last request:

```bash
shred -u "$platform_headers" 2>/dev/null || rm -f "$platform_headers"
```

## Architecture and data flow

```text
trusted server-side client
  -> platform bearer and bounded HTTP body
  -> agent-web PlatformCompatibility
  -> profile/session command-scope validation
  -> native AgentConnection over the local daemon socket
  -> agentd and leased worker
  -> native command result and/or ordered wire events
```

`agent-web` creates a fresh native client connection for each catalog,
capability, or command operation. It negotiates the current protocol and
forwards typed `ClientCommand` values. It does not implement business logic,
provider calls, session persistence, or approval decisions.

## Authentication, profile, and authority boundaries

- Only an exact bearer value is accepted; the configured secret is stored as a
  fixed-size comparison tag and redacted from debug output.
- Do not put the platform bearer in browser JavaScript, URLs, source code, or
  logs. Browser clients should use the authenticated Web interface or a narrow
  server-side facade.
- The URL profile, command-embedded profile, outer session, and
  command-embedded session must agree.
- Every referenced session must be present in the selected profile. Commands
  that require a session are rejected if the outer/embedded session is absent.
- Integration mutations must also agree with their embedded authority profile.
- `agentd` still enforces confirmation, credential, tool, plugin, control-lease,
  profile, and runtime policy after the HTTP boundary accepts a command.
- Request bodies are capped at 128 KiB before daemon connection.

The platform key authorizes access to all profiles visible to this `agent-web`
process. Per-user or per-tenant assignment is outside this route implementation
and must be enforced by a trusted upstream service before requests reach it.

## Failure and degraded behavior

Errors use `{"error":{"code":"...","message":"..."}}`. Expected classes
include `401 authentication_error`, `403 scope_denied`, `404
platform_api_disabled`, `413 payload_too_large`, `429
keith_capacity_exhausted`, and `503 keith_unavailable`.

`GET /platform/v1/health` always probes the daemon and returns `503` if it is
unavailable. After one successful catalog request, the catalog route may return
the last in-process cached profile/session summaries during a later daemon
failure. Treat that as discovery continuity, not proof that Keith is healthy;
use the health endpoint and live event state for readiness. Capabilities are not
served from that cache.

## Current limitations and status

- The bridge is a trusted, whole-instance bearer API; it is not a public
  multi-tenant authorization layer.
- There is no platform endpoint for browser login, credential value reads,
  provider-secret export, arbitrary filesystem access, artifact download, or
  raw daemon-socket tunnelling.
- There is no WebSocket variant; native command and event streaming use SSE.
- `ListProfiles` must use `/catalog`, and every other command remains subject to
  the URL profile and session checks.
- Availability of a `ClientCommand` also depends on the feature set returned by
  `/capabilities` and the selected profile's policy. A serialized command is not
  proof the daemon will admit it.
- The route startup/security test uses an unavailable daemon for its negative
  cases; it does not prove a real provider turn.

## Validate changes

The tests in `apps/agent-web/src/server/platform_compat.rs` cover exact bearer
authentication, redaction, and fail-closed capacity configuration.
`apps/agent-web/tests/platform_startup.rs` starts the packaged router and checks
unauthenticated access, unavailable-daemon behavior, and size limits. Scope
validation tests in `server.rs` cover profile/session disagreement and
installation-level command isolation.

For integration qualification, use a real daemon and profile to exercise
catalog, capabilities, create/submit/cancel, SSE command completion, event
resume, stale-cursor snapshot recovery, wrong-profile rejection, overload, and
daemon restart. Record any provider or infrastructure dependency that was not
available instead of treating route tests as end-to-end proof.

## Key source locations

- `apps/agent-web/src/server/platform_compat.rs` — platform routes, bearer, admission, and SSE
- `apps/agent-web/src/server.rs` — shared daemon bridge and profile/session validation
- `apps/agent-web/tests/platform_startup.rs` — packaged boundary startup tests
- `crates/protocol/src/lib.rs` — native command, result, event, snapshot, and cursor contracts
- `docs/reference/agent-connection.md` — generated wire schema
- `crates/daemon-core/src/` — authoritative command handling and profile/session ownership
