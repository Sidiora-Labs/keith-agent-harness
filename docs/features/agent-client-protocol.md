# Agent Client Protocol (ACP)

`keith-agent-acp` lets ACP-compatible editors and clients use a durable Keith profile
without bypassing Keith's native runtime. It translates ACP JSON-RPC sessions,
prompts, cancellation, and updates to `AgentConnection` commands and stores the
ACP-to-Keith session binding for later load, resume, fork, and close operations.

Stable ACP v1 is the default. Draft v2 is isolated behind both a compile-time
feature and a runtime switch so an experimental request cannot silently change
the semantics of a v1 session.

## Supported ACP v1 workflows

- protocol initialization and capability negotiation;
- creating, loading, resuming, forking, and closing durable sessions;
- prompting with text, image, audio, and embedded resource content;
- additional workspace directories admitted beneath configured roots;
- cancellation of the active prompt/session;
- assistant text, thought, plan, tool, tool-output, diff, usage, warning,
  failure, and terminal update projection;
- bounded client filesystem, terminal, and MCP facilities when explicitly
  enabled by process policy; and
- stdio, authenticated WebSocket, and authenticated HTTP/SSE transports.

The advertised agent capabilities include session load, additional directories,
fork, resume, close, prompt images/audio/embedded context, and MCP HTTP/SSE only
when an allowed network-host policy is configured.

## Run over stdio

An ACP process is pinned to one Keith profile and workspace. Obtain the profile
and workspace IDs from Keith's authenticated catalog, then configure the daemon
socket, ACP state, and admitted workspace roots:

```bash
bin/keith-agent-acp \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --state-root "$KEITH_DATA_ROOT/acp" \
  --profile PROFILE_ID \
  --workspace WORKSPACE_ID \
  --workspace-root /absolute/path/to/workspace
```

`--workspace-root` may be repeated. `--staging-root` defaults to
`STATE_ROOT/staging`. The stdio transport is newline-delimited UTF-8 JSON-RPC:
standard output is reserved for protocol frames, and the first frame must be a
single `initialize` request with an ID and exact protocol version.

Configure an editor or ACP client to launch the executable with those arguments
instead of wrapping it in a shell that mixes logs into standard output.

## Managed transports

Managed mode exposes WebSocket and split HTTP/SSE transports from one listener.
Read the bearer from an environment variable; do not pass it as a command-line
value:

```bash
export KEITH_ACP_BEARER="$(openssl rand -hex 32)"

bin/keith-agent-acp \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --state-root "$KEITH_DATA_ROOT/acp" \
  --profile PROFILE_ID \
  --workspace WORKSPACE_ID \
  --workspace-root /absolute/path/to/workspace \
  --transport managed \
  --listen 127.0.0.1:7350 \
  --bearer-token-env KEITH_ACP_BEARER
```

All managed routes require `Authorization: Bearer ...`:

| Method and path | Purpose |
| --- | --- |
| `GET /acp/ws` | Bidirectional UTF-8 JSON-RPC WebSocket |
| `PUT /acp/sse/{connection_id}` | Create a split HTTP/SSE connection |
| `POST /acp/sse/{connection_id}/messages` | Send one JSON-RPC frame |
| `GET /acp/sse/{connection_id}/events` | Receive and replay outbound frames |
| `DELETE /acp/sse/{connection_id}` | Close and remove the connection |

An SSE connection ID must be 1-128 ASCII letters, digits, `_`, or `-`. The
create response returns the exact message and event paths. Each posted body must
be one valid JSON value no larger than 2 MiB and receives `202 Accepted` after
it enters the connection.

## Client-facility policy

ACP clients do not receive ambient machine authority. The default process
policy allows bounded text-file reads inside the admitted session roots and
denies client writes and terminal execution.

Enable facilities narrowly by appending explicit policy options to the normal
invocation:

```bash
bin/keith-agent-acp \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --state-root "$KEITH_DATA_ROOT/acp" \
  --profile PROFILE_ID \
  --workspace WORKSPACE_ID \
  --workspace-root /absolute/path/to/workspace \
  --allow-client-write true \
  --allow-client-terminal true \
  --allow-client-executable /usr/bin/git \
  --allow-client-network-host tools.example.com \
  --allow-client-credential-ref docs-mcp-token
```

The executable, network-host, credential-reference, and workspace-root options
may be repeated. Enabling terminal capability without an executable allowlist
does not grant arbitrary execution. Terminal working directories must remain
inside admitted roots, environment names are validated, and output is bounded.

MCP server configuration supplied during session creation is intersected with
this process policy. Network endpoints must match an allowed host, credentials
must be references from the allowlist, server count and schemas are bounded,
and MCP schemas are admitted only after a health result. Reconnecting clients
may reduce durable capabilities but cannot widen them or replace an MCP server's
configuration under the same ID.

## Architecture and durable mapping

```text
ACP client
  -> ACP SDK v1 handler or separately gated v2 handler
  -> content/capability/permission validation
  -> AcpSessionBridge and durable ACP registry
  -> native AgentConnection
  -> agentd and leased worker
  -> native events and snapshot
  -> ACP session/update projection
```

Each ACP session record binds:

- the external ACP session ID to the native Keith session ID;
- the configured profile and workspace;
- canonical workspace/additional roots;
- the exact ACP protocol version;
- the negotiated client facilities and MCP configuration;
- the newest native resume cursor;
- prompt ordinal, digest, attachment IDs, and in-flight state; and
- fork ancestry and closed/ready/running/cancelling state.

Session creation and load attach to the native Keith session and project its
authoritative snapshot. Fork creates an independent Keith root from committed
context and is rejected while the source has an in-flight prompt. Close first
cancels outstanding work, then detaches and records the closed state.

## Prompt streaming, cancellation, and recovery

Prompt content is normalized into text and staged binary attachments before the
native `SubmitPrompt` command is sent. The default bounds are:

- 2 MiB of prompt text;
- 16 attachments;
- 25 MiB per attachment; and
- 50 MiB total attachment bytes.

The bridge stores a digest and command identity before submission. Repeating an
in-flight prompt with the same content can continue its durable operation;
different content is rejected while that prompt is active. Native events update
the stored cursor and are projected into ACP `session/update` notifications.
The driver polls/resumes until a terminal state is observed rather than treating
command acceptance as completion.

ACP request cancellation sends a native cancellation for that exact session and
returns the JSON-RPC cancellation error. Completed, failed, cancelled, and
exhausted native terminal states are preserved in ACP stop reasons and final
updates.

For managed HTTP/SSE, every outbound frame receives an increasing SSE event ID.
The server retains up to 512 frames. Reconnect with `?after=FRAME_ID` or
`Last-Event-ID`; an invalid or no-longer-replayable cursor fails explicitly. A
consumer that falls behind the retained broadcast window receives an error and
must recover through the durable ACP session load/resume path. The WebSocket
transport has no frame replay layer, but the same durable session APIs remain
available after reconnect.

## Authentication, profile, and authority boundaries

- Stdio authority belongs to the process owner that launched `keith-agent-acp` and
  selected its fixed profile, workspace, roots, and facility policy.
- Managed transport additionally requires an exact bearer on every route.
- A session cannot cross the process's configured profile or workspace.
- Additional directories are canonicalized and must remain within an admitted
  root; path escape and symlink substitution are rejected.
- Client tools remain subject to the intersection of negotiated client
  capability and configured Keith policy.
- Consequential operations still use Keith's permission bridge. A client may
  select only an option actually offered for the unchanged request, and
  persistent approval is not accepted for consequential actions.
- An ACP session is bound to one exact protocol version. It cannot be resumed
  through another version.

## Draft ACP v2

Draft v2 must be compiled and enabled explicitly:

```bash
keith_acp_target="$(mktemp -d /tmp/keith-acp-v2.XXXXXX)"
CARGO_TARGET_DIR="$keith_acp_target" CARGO_INCREMENTAL=0 \
  cargo build --locked --release \
  -p keith-agent-acp \
  --features unstable-acp-v2

"$keith_acp_target/release/keith-agent-acp" \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --state-root "$KEITH_DATA_ROOT/acp-v2" \
  --profile PROFILE_ID \
  --workspace WORKSPACE_ID \
  --workspace-root /absolute/path/to/workspace \
  --unstable-acp-v2 true
```

Keep that external target while the process is running, then remove the exact
directory after shutdown.

Both gates are required. Without them, a v2 initialize request receives an
explicit unsupported-version response; it is never downgraded to v1. V1 and v2
dispatch use separate handlers, and durable sessions reject version crossover.
Treat v2 as experimental and do not advertise it as the stable integration
contract.

## Current limitations and status

- One `keith-agent-acp` process serves one configured Keith profile and workspace.
- Stable interoperability is ACP v1; v2 remains a compile-time and runtime
  opt-in draft.
- Default client authority is read-only text access. Writes, terminal, network
  MCP, executables, and credential references require explicit allowlists.
- Managed transport is a separate listener and bearer boundary. It is not
  provided through `agent-web`.
- Binary WebSocket frames are rejected; protocol frames must be UTF-8 JSON text.
- Replay is bounded. Durable resume is required once the SSE frame window is
  exceeded.
- ACP projection intentionally exposes supported agent-session concepts, not
  every internal Keith daemon event or administrative surface.

## Validate changes

`apps/agent-acp/tests/protocol_process.rs` executes the real process and covers:

- malformed JSON-RPC recovery;
- exact v1 negotiation and post-initialize batch handling;
- refusal of unsupported protocol versions;
- managed HTTP/SSE bearer authentication, replay, and close;
- managed WebSocket frames and version refusal without downgrade; and
- the independently gated draft-v2 handler.

The `keith-acp` crate tests durable storage, bridge idempotency, event
projection, path and capability boundaries, MCP health/schema limits,
permission challenges, protocol routing, and transport replay. A full
qualification must additionally use a real ACP client, daemon, provider, prompt,
cancellation, process restart, session resume, and any enabled MCP/terminal
facility. Do not present the deterministic process suite as proof of an external
editor or provider that was not run.

## Key source locations

- `apps/agent-acp/src/main.rs` — v1 server, configuration, prompt driver, and update conversion
- `apps/agent-acp/src/managed_transport.rs` — WebSocket and HTTP/SSE routes
- `apps/agent-acp/src/client_facilities.rs` — ACP client capability and MCP conversion
- `apps/agent-acp/src/v2.rs` — separately compiled draft-v2 handler
- `apps/agent-acp/tests/protocol_process.rs` — real-process transport and negotiation tests
- `crates/acp/src/bridge.rs` — durable native-session bridge
- `crates/acp/src/capabilities.rs` — facility and MCP policy intersection
- `crates/acp/src/permission.rs` — permission challenge boundary
- `crates/acp/src/projector.rs` — native event-to-ACP update projection
- `crates/acp/src/protocol_router.rs` — exact version routing
- `crates/acp/src/store.rs` and `transport.rs` — durable session registry and replay transport
