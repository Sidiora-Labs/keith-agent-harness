# Terminal interface

`agent-tui` is Keith's interactive terminal client. It is a thin
`AgentConnection` client over the same profiles, sessions, commands, events,
and durable state as the Web interface. It can attach to an existing daemon,
start and supervise a local daemon, or connect to an authenticated remote
AgentConnection WebSocket.

## Supported workflows

The TUI supports conversation creation and switching, provider/model selection,
immediate prompts, queued prompts, steering, cancellation, retry, resume,
branching, confirmation decisions, goals, children, schedules, memory search,
exports, background policy, diagnostics, self-evolution history, and service
inspection. Service views cover channel accounts, connected apps, plugins, ACP
connections, computers, recordings, task recipes, and harness repairs.

Assistant deltas, committed messages, tool calls, work state, token usage, and
terminal outcomes are rendered from the shared UI projection. A prompt appears
in the transcript as soon as it is queued for dispatch; it is then reconciled
with the daemon's committed event instead of being duplicated.

## Start the TUI

Attach to a daemon that is already running:

```bash
bin/agent-tui --socket "$KEITH_DATA_ROOT/agentd.sock"
```

From a source checkout, use an external Cargo target if the binary has not been
built yet:

```bash
(
  keith_target="$(mktemp -d /tmp/keith-tui.XXXXXX)"
  trap 'find "$keith_target" -depth -delete' EXIT
  CARGO_TARGET_DIR="$keith_target" CARGO_INCREMENTAL=0 \
    cargo run --locked -p keith-agent-tui --bin agent-tui -- \
    --socket "$KEITH_DATA_ROOT/agentd.sock"
)
```

To let the TUI start and supervise sibling `agentd` and `agent-worker` binaries:

```bash
bin/agent-tui --data-root /absolute/path/to/keith-data
```

With no connection option, the TUI discovers the platform data paths and uses
supervised-local mode. `--daemon-executable` and `--worker-executable` override
the sibling binaries. The supervised daemon uses a 15-minute idle timeout and
is terminated when the client that started it exits.

Open a particular durable conversation at startup:

```bash
bin/agent-tui \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --session SESSION_ID
```

Remote mode requires a direct AgentConnection WebSocket and reads its bearer
from an environment variable:

```bash
export KEITH_REMOTE_TOKEN='replace-with-the-remote-bearer'
bin/agent-tui \
  --remote wss://keith.example/agent-connection \
  --token-env KEITH_REMOTE_TOKEN
unset KEITH_REMOTE_TOKEN
```

The browser subscription route exposed by `agent-web` is not a general remote
TUI endpoint. The remote URL must implement the authenticated bidirectional
AgentConnection transport.

Use `--color truecolor`, `--color 256`, `--color none`, or
`--color contrast` to select a palette. `--reduced-motion` disables the animated
activity indicator. The process refuses non-interactive standard input or
output.

## Everyday controls

| Key | Action |
| --- | --- |
| `Enter` | Send now, or queue for the next turn boundary while Keith is working |
| `Alt-Enter` | Insert a newline |
| `Ctrl-P` | Open the searchable command palette |
| `Ctrl-S` | Switch conversations |
| `Ctrl-L` | Start a new conversation |
| `Ctrl-K` | Steer the active turn with the current draft |
| `Ctrl-X` | Cancel the active turn |
| `Ctrl-R` | Retry the last prompt |
| `Ctrl-B` | Branch from the latest committed entry |
| `Ctrl-U` | Resume the current conversation |
| `Ctrl-T` | Toggle compact and expanded tool detail |
| `Ctrl-Y` | Open saved context |
| `Ctrl-E` or `Ctrl-G` | Edit the draft with `$VISUAL` or `$EDITOR` |
| `PageUp` / `PageDown` | Scroll the transcript |
| `Esc` | Interrupt or close the active overlay |
| `Ctrl-D` | Exit |
| `Ctrl-C` | Clear a draft, then cancel active work, then exit when idle |

Type `?` into an empty composer for the complete shortcuts view. The overlays
are searchable; use arrow keys to select, `Enter` to activate, `Tab` to move to
the next view, and `Esc` to close.

## Slash commands

The command palette provides the common forms. Frequently used commands are:

```text
/new                         start a new durable conversation
/sessions                    choose a conversation
/model PROVIDER [MODEL]      select the provider and optional model
/resume                      resume the attached conversation
/stop                        cancel the active turn
/goal OBJECTIVE              create a bounded goal
/child OBJECTIVE             delegate bounded child work
/memory QUERY                search profile-scoped memory
/schedule SECONDS PROMPT     create an interval schedule
/export [jsonl|markdown|bundle]
/background disabled|suggest|confirm|bounded
/approvals                   review pending decisions
/work                        inspect goals, children, and other work
/services                    inspect all external services
/evolution                   review evolution status and history
/diagnostics                 inspect connection and session state
/details                     toggle tool detail
/help                        show shortcuts
/quit                        exit
```

Additional commands manage child messages, schedule pause/resume/delete,
branch selection, goal/child cancellation, service testing and restart,
computer control release, recording stop, and harness reversal. Use the command
palette instead of guessing identifiers or action syntax.

## Architecture and state

```text
terminal input
  -> TuiApp command/state model
  -> bounded command dispatcher
  -> local framed JSON or authenticated WebSocket AgentConnection
  -> agentd
  -> ordered events, snapshots, terminal frames, command results
  -> ProjectionReducer
  -> ratatui renderer
```

The TUI owns only transient presentation state: composer contents, selection,
scroll position, overlay filters, pending prompts, and connection status. The
daemon owns sessions, messages, approvals, work, tools, integrations, and
terminal results. Starting a new conversation clears the displayed prior
transcript immediately, but the new daemon snapshot decides the final state.

The composer is bounded to 64 KiB, the input history to 200 entries, log output
to 512 lines, and pending commands to 128. Bracketed paste is enabled while the
TUI is active and disabled when the terminal is restored. Control bytes in
untrusted content are rendered safely rather than executed.

## Authentication, profile, and authority boundaries

- Local attach relies on access to the owner-controlled daemon socket and still
  performs the protocol hello.
- Remote mode sends a bearer authorization header read from `--token-env`; it
  never accepts the token itself as a CLI argument.
- The daemon negotiates supported features before commands are dispatched.
- The attached session determines profile scope. Memory and integration
  commands copy that authoritative profile into the native command.
- Approval views emit only `allow_once` or `deny` decisions for an existing
  confirmation. The TUI does not create confirmation authority.
- Evolution enablement and protected restore actions remain installation-owner
  requests; displaying them does not bypass daemon policy.

## Reconnect and failure behavior

Transport work runs outside the render loop. When a connection changes, the TUI
marks each pending command as uncertain, reconnects with the same client
identity, and re-sends the same command identities so daemon idempotency can
resolve duplicates safely. Failed reconnect attempts are shown and retried with
bounded 500 ms waits until shutdown. After reconnection, the attached session is
resumed to recover its authoritative projection.

The status line distinguishes offline, connected, and reconnecting states and
shows completed, failed, cancelled, or exhausted terminal outcomes. It also
shows the active tool and token count when space permits. A command rejection,
scope failure, or connection failure is displayed as an error; the TUI does not
convert a sent command into success before the daemon result arrives.

## Current limitations and status

- `agent-tui` requires a real interactive terminal.
- File attachments are not currently added through the composer.
- Remote operation requires a separately deployed AgentConnection WebSocket;
  `agent-web` does not supply that general command socket.
- Provider credentials are configured through `agent-cli` or the authenticated
  Web settings flow, not typed into the TUI.
- Terminal rendering and model behavior vary independently. A correct screen
  snapshot does not prove provider or daemon execution.

## Validate changes

Run focused tests and strict Clippy for `keith-agent-tui`. The terminal test
matrix exercises wide, narrow, no-color, high-contrast, reduced-motion, active,
failed, and overlay states. The Unix PTY journey covers a real connection,
prompt ordering, paste, resize, external-editor handoff, signal handling, and
terminal restoration. Connection tests cover real protocol attach/reconnect and
profile-scoped integration messages.

Relevant commands are documented in the repository-wide `AGENTS.md`; keep
Cargo output outside the checkout. Manual qualification should also attach to a
running daemon, create a new session, send a prompt, observe live work, cancel
or steer a turn, and force one reconnect.

## Key source locations

- `apps/agent-tui/src/main.rs` — terminal lifecycle and event loop
- `apps/agent-tui/src/lib.rs` — application state, key bindings, commands, and projections
- `apps/agent-tui/src/connection.rs` — local, supervised, remote, dispatch, and reconnect paths
- `apps/agent-tui/src/render.rs` — transcript, composer, activity, status, and overlays
- `apps/agent-tui/tests/terminal_matrix.rs` — renderer and real PTY journeys
- `apps/agent-tui/tests/terminal_compatibility.csv` — published terminal cases
- `crates/ui-model/src/lib.rs` — shared authoritative projection reducer
- `crates/protocol/src/lib.rs` — command and event contracts
