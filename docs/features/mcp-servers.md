# MCP servers

Keith's MCP manager turns explicitly configured Model Context Protocol servers
into bounded, profile-scoped tools. It owns server configuration, credential
references, schema refresh and relevance selection, session admission, health,
timeouts, transport cleanup, and tool calls. Remote output remains untrusted
tool data and receives no authority from being returned by an MCP server.

This subsystem is distinct from Keith's ACP server. ACP clients can negotiate
session-scoped MCP facilities through the ACP boundary; the core runtime can
also register daemon-owned MCP servers for selected profiles.

## Current status

The `keith-mcp` library implements stdio and HTTP MCP transports and has focused
in-crate tests covering real child processes, loopback HTTP, credentials,
isolation, bounds, reconnect, malicious output, and cleanup. There is no
standalone MCP configuration CLI, public configuration file loader, Web setup
screen, or dedicated qualification manifest today.

Runtime profiles contain `enabled_mcp_servers`, but those names reference
servers already present in the daemon-owned `McpManager`. The assembled local
runtime opens the manager and consumes configured schemas; it does not currently
load arbitrary operator server definitions from the normal profile file.
Composio can programmatically configure a profile-only MCP server, while ACP has
its own separately negotiated session-scoped MCP path.

## Core server configuration

`McpServerConfig` requires:

- a lowercase ASCII server ID containing letters, digits, or `-`;
- one transport: stdio or HTTP;
- a non-empty set of enabled profile IDs;
- an optional credential reference with explicit placement;
- allowed filesystem roots or network hosts appropriate to the transport;
- positive timeout, request-byte, response-byte, and tool-count limits.

Stdio configuration contains an absolute executable, literal arguments, an
optional working directory, and a minimal explicit environment. The executable
must be an existing file. A working directory must canonicalize beneath one of
the configured filesystem roots. A stdio credential may be placed only in a
named environment variable.

Core HTTP configuration accepts plain `http` only and requires the endpoint host
in `allowed_network_hosts`. Static headers are validated against line breaks. An
HTTP credential may be placed only in a named header. This transport is designed
for explicitly admitted local or controlled HTTP boundaries; it is not a
general-purpose arbitrary HTTPS client. Composio reaches hosted HTTPS MCP
through its bounded stdio proxy instead.

Credentials are `CredentialRef` values owned by `CredentialOwner::Mcp(server_id)`.
The encrypted store resolves the value only for a request. Server state contains
the reference and placement, not secret bytes.

## Architecture and runtime flow

```text
trusted configuration
  -> validate transport, roots/hosts, credential owner, and limits
  -> persist config and unknown health
  -> tools/list over bounded transport
  -> validate, sort, digest, and version schema cache
  -> select profile-enabled relevant schemas under context budget
  -> register mcp_<server>_<tool> with the session ToolManager
  -> open exact (Keith session, MCP server) binding
  -> tools/call with bounded JSON arguments
  -> normalized content/isError result
```

Server configurations, schema caches, and health statuses persist in
`mcp-state.json`. Runtime session bindings are intentionally in memory and are
reopened from the profile/server relationship when the Keith session is
assembled after restart.

Schema refresh sends `tools/list`, requires a well-formed tools array, enforces
the configured count and response bounds, sorts by tool name, and stores a
digest. The cache version increments only when that digest changes. Relevance
selection combines lexical overlap with an optional integer embedding score and
stops at the caller's exact serialized-byte budget.

The local runtime exposes selected schemas as `mcp_<server-id>_<tool-name>`.
Those tools still pass through Keith's normal installation/profile policy,
confirmation, timeout, output, and child-delegation boundaries. The guest
kernel can request the same configured server through
`rlm.call_mcp(server_id, tool_name, arguments_dict)`; raw protocol methods such
as `tools/list` are not the bridge API.

## ACP session-scoped MCP

ACP clients may supply MCP servers when creating a session, but Keith intersects
them with the `agent-acp` process policy:

- stdio requires admitted terminal capability and an allowlisted executable;
- HTTP and SSE require credential-free HTTPS URLs on allowlisted hosts;
- credential references must be explicitly allowlisted;
- server count, arguments, environment, and schemas are bounded;
- tools stay unavailable until a healthy result is recorded; and
- reconnect may reduce capability but cannot replace a server's configuration
  under an existing ID or widen authority.

See [Agent Client Protocol](agent-client-protocol.md) for the process options and
managed transport setup. ACP's HTTP/SSE allowance is an ACP client-facility
policy and should not be confused with the core `McpTransport::Http` validator.

## Security boundaries

- A server is enabled per exact profile. Opening a session for another profile
  returns `ProfileDenied`.
- A tool call needs an existing `(SessionId, server_id)` binding and a tool name
  already present in the server's admitted schema cache.
- Credentials are resolved by exact MCP ownership and placed only in the
  transport location declared by validated configuration.
- Stdio starts the exact executable with literal arguments, clears the ambient
  environment, discards stderr, and creates an owned process group.
- HTTP requires an allowlisted host and rejects malformed header names/values.
- Request, response, tool-count, schema-context, session-count, and timeout
  bounds are deterministic host policy.
- Descriptions, schemas, content arrays, `isError`, and remote errors are
  untrusted. They cannot approve another tool, modify profile policy, or bypass
  the normal ToolManager.
- Do not put bearer values in `McpServerConfig.environment`, static headers,
  profile JSON, logs, or docs. Use the encrypted credential reference.

## Failure and recovery

- Configuration fails before persistence if any ID, owner, transport, path,
  host, placement, or bound is invalid.
- `refresh_schema` marks a successful server healthy. `reconnect` increments its
  generation, repeats the bounded schema journey, and persists an unhealthy
  status with a safe error if it fails.
- Stdio requests wait for one newline-delimited JSON response. Timeout kills the
  owned process group and reaps the child; output beyond the configured limit is
  rejected.
- HTTP applies read/write timeout and response bounds. Transport, malformed
  JSON-RPC, remote error, authentication, and size failures remain distinct.
- Closing a Keith session removes all of its MCP bindings. Daemon shutdown can
  clear all bindings; persisted configuration and schemas remain available for
  deliberate reopen.
- A state-changing MCP call uses `CheckBeforeRetry` in the runtime. A lost or
  malformed result must not be treated as proof that the remote side had no
  effect.

## Current limitations

- There is no supported general operator command to add, remove, or inspect a
  daemon-owned MCP server. Integrators currently use the Rust API, Composio
  connector, or ACP's session-scoped facility.
- The core HTTP transport requires plain `http` plus an allowlisted host; hosted
  HTTPS needs a constrained bridge or the separately governed ACP path.
- The manager implements the `tools/list` and `tools/call` subset used by Keith.
  It is not a complete general MCP client implementation.
- Core manager configuration/cache state is durable; live child processes and
  session bindings are not resumed as processes after restart.
- There is no standalone MCP qualification artifact. Focused tests are not
  proof of a specific third-party server or credentialed service.

## Validate changes

Focused manager validation:

```bash
cargo test -p keith-mcp --locked
cargo clippy -p keith-mcp --all-targets --no-deps --locked -- -D warnings
```

Changes to runtime exposure also require focused `keith-local-runtime` tests.
ACP-supplied MCP changes require the ACP capability and real-process suites;
Composio transport changes require `keith-composio` tests. Use an external
disposable Cargo target.

Cover invalid configuration, profile denial, wrong credential owner and
placement, schema change, relevance budget, unknown tools, response injection,
timeouts, output floods, process-group cleanup, reconnect health, restart, and
unknown side-effect outcomes. Add a credentialed external journey before
claiming compatibility with a named server.

## Key source locations

- `crates/mcp/src/lib.rs` — core manager, transports, credentials, cache, health,
  sessions, selection, calls, and tests
- `crates/local-runtime/src/lib.rs` — profile enablement, runtime tool names,
  ToolManager registration, and guest-kernel bridge
- `crates/configuration/src/lib.rs` — profile `enabled_mcp_servers`
- `crates/credentials/src/lib.rs` — MCP-owned credential references
- `crates/composio/src/lib.rs` — programmatic profile-only MCP configuration
- `crates/composio/src/bin/keith-composio-mcp.rs` — hosted HTTPS proxy
- `crates/acp/src/capabilities.rs` — ACP session policy intersection
- `apps/agent-acp/src/client_facilities.rs` — ACP SDK facility conversion
- `.mcp.json` — contributor-tool configuration for Codify, not Keith runtime configuration
