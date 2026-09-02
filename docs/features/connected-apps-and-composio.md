# Connected apps and Composio

Keith's Composio connector is a profile-scoped control and tool-execution layer
for hosted application integrations. It manages connected-account identity,
OAuth completion, exact tool allowlists and risks, short-lived direct-tool MCP
sessions, approvals, audit, cancellation, revocation, and deletion without
exposing provider credentials to the model or browser.

## Current status

The connector library is implemented and has checked-in local qualification
evidence. The current result is
`local_conformance_passed_external_accounts_blocked`: real loopback HTTP and MCP
journeys passed when the evidence was recorded, but Gmail, GitHub, and Slack
accounts were not exercised against Composio because project credentials, auth
configuration, callback verification, user consent, and owner authorization
were unavailable.

There is not yet a supported end-user Composio setup path in the assembled Keith
applications:

- `ComposioConnector` is not mounted by `agentd` or `local-runtime`;
- the Rust protocol does not define the `connected_apps` command used by the
  standalone TypeScript data layer;
- `agent-web` does not expose `/api/connected-apps/commands`; and
- `ConnectedAppsPanel` has component tests but is not mounted in `KeithApp`.

The daemon's generic `IntegrationService::ConnectedApp` projection records
authority and lifecycle state, but it does not execute the Composio connector.
Enabling the `connected_apps` service group therefore does not, by itself,
configure Composio or make the browser flow operational.

## Implemented connector lifecycle

At the library boundary, the intended sequence is:

1. Store the Composio project API key in `EncryptedCredentialStore` under a
   `CredentialRef` owned by `Tool("composio-control-plane")`.
2. Open `ComposioConnector` with an HTTPS Composio control-plane base URL,
   positive resource limits, the credential reference, and a durable state root.
3. Set one `ProfileAppPolicy` containing exact toolkit names, exact tool names,
   risk classifications, and a context-schema byte ceiling.
4. Call `begin_connect` with an exact account-change `ExternalAction` and
   `AuthorityBoundary`. Keith receives a short-lived provider authorization URL
   and stores a connecting account identity.
5. After OAuth returns, call `complete_connect_callback` with the session URI.
   The connector asks Composio to complete authentication, verifies the same
   stable Keith profile and provider account/toolkit, then refreshes the account.
6. Create a short-lived direct-tools session, bind it to `McpManager`, refresh
   and validate the schema, then open the exact Keith session.
7. Discover only policy-allowed tools under the profile context budget and call
   one tool against one explicitly selected connected account.
8. Revoke the upstream account before deleting its local identity. Delete the
   hosted tool session separately when it is no longer needed.

These are Rust integration APIs, not currently public CLI or Web setup commands.

## Policy and account selection

`ProfileAppPolicy` is an allowlist, not discovery-by-default. It maps each
toolkit to exact tool names and `ActionRisk` values, and caps the schema bytes
that may enter model context. A policy cannot be changed while its profile has
an active provider session; create a replacement session after changing policy.

Multiple accounts per toolkit are supported, but every tool call must include a
Keith `ConnectedAccountId`. The connector injects the provider account identity
after approval. Caller arguments cannot override it. Selection precedence is
stored for presentation and deterministic ordering, not used as permission to
guess an account for a consequential action.

Composio sessions are configured with connection management, multi-execute,
search, and remote sandbox disabled. Keith retains control over account changes,
tool selection, execution, and authorization.

## Architecture and data flow

```text
owner-approved account change
  -> Keith profile/toolkit policy
  -> Composio control plane using server-side credential
  -> verified OAuth callback and refreshed account
  -> short-lived direct-tools provider session
  -> profile-only McpManager registration
  -> bounded schema discovery
  -> exact account + tool + risk authority check
  -> MCP tools/call
  -> recursive redaction and bounded result
  -> content-free append-only audit
```

The connector persists policies, safe account state, and session state in
`composio-state.json`. Action outcomes are appended to
`composio-audit.jsonl`. Browser projections omit provider user IDs, provider
account IDs, hosted MCP endpoints/server IDs, and bearer credentials.

Production HTTPS MCP calls go through the `keith-composio-mcp` stdio bridge.
The bridge receives only the endpoint and numeric bounds as arguments and reads
the copied MCP credential from `KEITH_COMPOSIO_MCP_API_KEY`. Plain HTTP is
accepted only for loopback qualification servers.

## Credentials, approvals, and security

- The control-plane API credential must have the exact tool owner
  `composio-control-plane`. The hosted MCP copy is stored under an MCP-specific
  owner and never written to connector state.
- The Composio control-plane base is restricted to allowed Composio HTTPS hosts;
  redirects, embedded credentials, fragments, arbitrary ports, and SSRF targets
  are rejected. Hosted MCP endpoints receive the same explicit validation.
- Connecting, completing a callback, invoking a tool, revoking, and deleting
  all validate the profile, session, capability, risk, target, payload digest,
  approval, expiry, cancellation identity, and audit identity appropriate to
  the action.
- The returned provider profile, user, session, account, and toolkit identities
  must match the values Keith issued. Substitution is refused.
- Tool arguments are bounded before provider account injection. Results are
  recursively redacted and bounded before they are returned.
- Durable state and temporary replacement files must be ordinary files;
  symlink-backed state and unsafe roots are rejected.
- Browser code rejects projections containing credential-like or provider-only
  keys and accepts authorization links only over credential-free HTTPS.

## Failure and recovery

- A connecting account has a short-lived authorization link. Expired, replayed,
  mismatched, or non-active callbacks fail without activating it.
- `resume_session` verifies that an unexpired provider session still belongs to
  the same profile and retains the same provider session identity. An expired
  session is marked interrupted and must be recreated.
- Schema refresh refuses tools outside the configured policy. Missing bridge,
  failed authentication, timeouts, malformed provider results, and size limits
  remain typed failures.
- Cancellation is checked before and after the MCP call, with a truthful audit
  outcome. A provider-side effect that raced cancellation must not be rewritten
  as a clean local success.
- Account revocation must succeed upstream before local deletion is allowed.
  A failed upstream deletion preserves the local record for reconciliation.
- Deleting a provider session overwrites the copied MCP credential with a
  revoked value before removing local session state.
- Connector state is written atomically. Corrupt or identity-substituted state
  fails closed on open rather than being silently repaired.

## Current limitations

- Gmail, GitHub, Slack, and every other real Composio account remain externally
  unqualified in the current evidence.
- The connector is a library subsystem today. There is no stable Composio CLI,
  daemon command, credential-setup wizard, callback HTTP route, or mounted Web
  control surface.
- The TypeScript connected-app components describe the planned browser contract,
  but component tests do not prove daemon or provider integration.
- Only exact tools already listed in `ProfileAppPolicy` can be discovered or
  invoked. Composio search and multi-execute are intentionally disabled.

## Validate changes

The recorded focused gates are:

```bash
cargo test -p keith-composio --locked
cargo clippy -p keith-composio --all-targets --all-features --no-deps --locked -- -D warnings
corepack pnpm --dir apps/agent-web/ui test
```

Connector tests use real loopback HTTP and MCP transports to cover account
linking, callback identity, session create/resume/expiry, schema policy, exact
account injection, approvals, cancellation, revocation, deletion, redaction,
SSRF refusal, symlink-resistant persistence, and cross-profile denial. They do
not prove a live Composio tenant or OAuth provider.

A complete product qualification also needs the missing daemon/protocol/Web
wiring plus an authorized real account journey through callback, read, write,
expiry, restart, revoke, and delete. Record the result in
`evidence/composio/qualification.json` without converting blocked external
inputs into a pass.

## Key source locations

- `crates/composio/src/lib.rs` — connector, policy, lifecycle, MCP binding,
  authority, audit, redaction, and persistence
- `crates/composio/src/bin/keith-composio-mcp.rs` — bounded HTTPS-to-stdio bridge
- `crates/composio/src/tests.rs` — real loopback control-plane and MCP journey
- `crates/composio/tests/qualification.rs` — SSRF, credential, storage, and
  evidence checks
- `crates/mcp/src/lib.rs` — normalized MCP manager used by the connector
- `crates/platform-contracts/src/lib.rs` — external action and approval contracts
- `apps/agent-web/ui/lib/apps.ts` — browser-safe planned command/data contract
- `apps/agent-web/ui/components/apps/ConnectedAppsPanel.tsx` — unmounted component
- `crates/daemon-core/src/integrations.rs` — generic connected-app lifecycle state
- `evidence/composio/qualification.json` — current qualification truth
