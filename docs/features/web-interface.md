# Web interface

Keith's Web interface is the full browser client for the same durable agent used
by the TUI, desktop lifecycle, APIs, channels, and ACP clients. It does not run a
second agent in the browser. The React application sends typed commands to
`agent-web`; `agent-web` authenticates and scopes them before forwarding them to
`agentd` over the native `AgentConnection` protocol.

## What you can do

The current interface supports:

- creating, selecting, resuming, and branching durable conversations;
- sending a prompt, queuing work at the next turn boundary, steering an active
  turn, cancelling it, and retrying the last prompt;
- reading incremental assistant text, tool activity, goals, plans, child-agent
  work, schedules, waits, usage, warnings, and terminal outcomes;
- approving or denying daemon-issued confirmations;
- creating goals and bounded child-agent work;
- searching profile-scoped memory and preparing portable conversation exports;
- selecting a provider/model and configuring background-work policy;
- inspecting channels, connected apps, plugins, ACP connections, computers,
  recordings, task recipes, and harness-repair resources;
- using the Computer stage, control leases, and task-teaching views when those
  services are enabled; and
- reviewing self-evolution status, evidence, promotion history, and available
  owner actions.

Some integration setup buttons are intentionally unavailable until the daemon
has issued the exact approval required for the operation. A disabled control is
not an implied capability.

## Run it from a source checkout

Install the pinned dependencies, set a Web password and one supported provider
credential, then start the local stack:

```bash
./keith setup
export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'
export OPENAI_API_KEY='your-provider-key'
./keith dev
```

Open <http://127.0.0.1:7341> and sign in with
`KEITH_WEB_LOGIN_SECRET`. `./keith dev` builds `agentd`, `agent-worker`,
`agent-web`, and `agent-cli`; stores development state beneath
`KEITH_DEV_DATA_ROOT` (or the platform data directory); enables the channel,
ACP, plugin, connected-app, computer, and teaching service groups; and stops
the child processes when the wrapper exits.

For an installed release, run `agentd` first and point `agent-web` at its local
socket and the packaged `web` directory:

```bash
export KEITH_DATA_ROOT=/absolute/path/to/keith-data
export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'

bin/agent-web \
  --bind 127.0.0.1:7341 \
  --origin http://127.0.0.1:7341 \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --asset-root "$PWD/web" \
  --credential-root "$KEITH_DATA_ROOT/credentials" \
  --login-secret-env KEITH_WEB_LOGIN_SECRET
```

The complete installed-daemon and provider setup is in
[Install and lifecycle](../installation.md).

## Browser routes

| Route | Purpose |
| --- | --- |
| `GET /` and `GET /login` | Serve the packaged browser application |
| `POST /auth/session` | Exchange the login password for an HTTP-only browser session |
| `GET /api/bootstrap` | Load the negotiated protocol, CSRF token, profiles, and sessions |
| `POST /api/profiles/{profile}/commands` | Execute a profile-scoped native command; optionally stream it with SSE |
| `GET /api/events/{profile}/{session}` | Open the session WebSocket with an optional resume cursor |
| `POST /api/profiles/{profile}/credentials` | Store a provider credential through the write-only browser flow |
| `POST /api/evolution/commands` | Send the deliberately narrow installation-level evolution commands |
| `GET /assets/ui/{path}` | Serve immutable, path-validated Next.js assets |

The `/v1` and `/platform/v1` routes on the same process are separate APIs with
separate bearer credentials. Browser cookies do not authorize either API.

## Architecture and data flow

```text
React UI
  -> authenticated HTTP command or session WebSocket
  -> agent-web origin, CSRF, profile, and session checks
  -> AgentConnection over the local daemon socket
  -> agentd and the leased worker
  -> event, snapshot, terminal, and command-result frames
  -> browser projection reducer
```

The bootstrap response is a catalog, not a second state store. Conversation
truth comes from daemon snapshots and ordered events. The reducer tracks a
`generation` and `sequence`, ignores duplicates and older generations, and
requests a fresh snapshot if it detects a sequence gap or malformed projection.
New-conversation and pending-prompt states are shown immediately while the
daemon command is in flight, then reconciled with the authoritative result.

## Authentication and authority

- The login secret is read from the environment named by
  `--login-secret-env`; the value is never accepted as a CLI argument.
- A successful login creates an in-memory, eight-hour `HttpOnly`,
  `SameSite=Strict` session cookie. HTTPS origins also receive `Secure`.
- Login, mutation, credential, and WebSocket requests must carry the configured
  exact origin. Mutations additionally require the bootstrap CSRF value.
- Browser mutations are limited to 24 per second per browser session. Request
  bodies are limited to 128 KiB.
- `agent-web` rejects malformed profile IDs, requires every referenced session
  to belong to the URL profile, and rejects disagreement between outer and
  embedded command scopes.
- Provider secrets go directly to the encrypted credential store. The UI does
  not retain their values in browser storage.
- Integration mutations carry profile, session, principal, capability, risk,
  and effect information. The browser cannot manufacture an approval by
  enabling a disabled button.

Keep the default loopback listener unless a trusted reverse proxy supplies TLS
and network policy. The browser server's exact-origin check is not a substitute
for either.

## Streaming, reconnect, and failure behavior

Prompt commands request `text/event-stream`. The stream carries native event,
snapshot, and terminal frames followed by the matching command result. A
missing terminal result, malformed event, or explicit `stream_error` is shown as
a recoverable failure rather than guessed into success. Profile command
submissions retry network failures and server-side failures up to three attempts
with bounded backoff; client errors are not retried.

The long-lived conversation feed is a WebSocket. On disconnect it retries with
400 ms, 800 ms, 1.6 s, 3.2 s, 6.4 s, then 8 s bounded backoff. It reconnects
from the newest applied generation and sequence. If the cursor is stale or an
event gap is detected, the client discards the partial projection and asks for
an authoritative snapshot. The interface distinguishes opening, ready,
working, reconnecting, unavailable, cancelled, failed, exhausted, and completed
states.

## Current limitations and status

- The browser is a client of `agentd`; it cannot operate while the daemon socket
  is unavailable.
- Browser sessions are held in `agent-web` memory. Restarting that process
  requires the user to sign in again; durable Keith sessions are unaffected.
- File attachment staging is present in the native protocol but is not exposed
  by the current composer.
- External account connection, plugin installation, and destructive removal
  remain approval-gated; several setup controls are deliberately read-only.
- The Web interface is packaged as a statically exported Next.js application
  served by the Rust process. Running the Next.js development server alone does
  not provide the Keith backend.
- A component test or successful static build does not prove login, daemon,
  provider, streaming, or restart behavior.

## Validate changes

For UI-only work:

```bash
corepack pnpm --dir apps/agent-web/ui check
corepack pnpm --dir apps/agent-web/ui test
corepack pnpm --dir apps/agent-web/ui build
```

For server or transport work, also run focused Rust tests and strict Clippy for
`keith-agent-web`. The ignored Chromium journey in
`apps/agent-web/tests/browser_journey.rs` requires a running real Keith stack,
Playwright, the Web origin, and the login secret. Use that journey for changes
to login, send/receive, streaming, reconnect, or security behavior; do not
describe a UI test as live-browser qualification.

## Key source locations

- `apps/agent-web/ui/components/KeithApp.tsx` — application shell and workflows
- `apps/agent-web/ui/lib/keith.ts` — command client, projection reducer, SSE, and WebSocket URLs
- `apps/agent-web/src/server.rs` — routes, daemon bridge, command scope, and static assets
- `apps/agent-web/src/security.rs` — login cookie, exact-origin, CSRF, and mutation limits
- `crates/ui-model/src/lib.rs` — shared client projection model
- `crates/protocol/src/lib.rs` — native commands, events, snapshots, and cursors
- `apps/agent-web/ui/tests/` — component and surface tests
- `apps/agent-web/tests/browser_journey.*` — live browser journey
- `apps/agent-web/tests/platform_startup.rs` — packaged server and API-boundary startup test
