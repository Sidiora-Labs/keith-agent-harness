# Runtime and sessions

Keith runs as a durable local service rather than as state held by one chat
window. The daemon owns sessions, action queues, branches, lifecycle state, and
recovery. Web, terminal, desktop, API, channel, and ACP clients connect to that
same state through versioned commands and events.

This design lets a conversation survive a client disconnect or process
restart. It does not make every request magically exactly-once: clients still
need to wait for authoritative acceptance and handle an interrupted transport.

## What you can do

- Create a root conversation and attach another client to it later.
- Resume an existing session instead of copying its transcript into a new one.
- Create and select branches without rewriting the original history.
- Send a normal prompt, steer a running turn, or cancel work explicitly.
- Observe pending, running, waiting, compacting, paused, failed, and archived
  states through the same protocol used by the other clients.
- Reconnect after a daemon or worker restart and rebuild the current projection
  from durable events.
- Export a conversation and archive completed child sessions without granting
  clients direct access to the session store.

The terminal client exposes session commands such as `/new`, `/sessions`, and
`/resume`. The Web interface provides the same underlying operations through
its conversation list and new-chat flow.

## Architecture and data flow

```text
Web / TUI / API / channels / ACP
              |
      authenticated command
              v
        agentd control plane
              |
     per-session actor mailbox
       |                  |
 durable action queue     leased worker
       |                  |
       +------ events ----+
              |
 append-only session store
              |
 snapshots and projections -> connected clients
```

Every root or durable child uses the same `SessionIdentity` model. A session
actor serializes changes through a bounded mailbox; callers do not update the
snapshot or transcript directly. Inputs from interactive clients, channels,
schedules, children, steering, awareness, autonomous continuation, and
self-evolution are normalized into durable actions with an explicit priority
and delivery point.

The agent loop builds a provider request from committed context, consumes the
provider stream, executes authorized tools through the worker, and commits only
complete results. Partial streamed text is display data, not final history. A
tool call is paired with an outcome before the next durable turn is accepted.

The session store is append-only and checksummed. Branch manifests point to
history rather than editing it in place. Snapshots accelerate reconstruction,
but the event log remains the recovery source of truth.

## Running and connecting

Use the repository wrapper for a normal development instance:

```bash
./keith setup
./keith dev
```

For complete setup, process lifecycle, data-root, socket, and login options,
see [Install and run Keith](../installation.md). Clients should connect to the
socket or HTTP endpoint printed by the running instance instead of starting a
second independent Keith identity.

The most important runtime locations are chosen by the daemon:

- the data root contains durable profiles, sessions, credentials, and runtime
  state;
- the workspace root is the directory Keith may inspect or change through
  workspace capabilities;
- the local socket is the native control path used by terminal and desktop
  clients.

Do not point two independently managed daemon processes at the same data root.
Writer leases protect individual session logs, but they are not a substitute
for a single lifecycle owner.

## Authority and security

- The daemon owns durable truth. Clients send typed commands and render typed
  events; they do not invent session or approval state locally.
- Every command is scoped to an authenticated profile and client attachment.
  An unattached client cannot mutate a session merely by knowing its ID.
- Child sessions inherit a bounded delegation from their parent. They do not
  gain new tools, credentials, or filesystem access by being forked.
- Provider output, tool output, retrieved memory, external messages, and files
  are untrusted content. None of them can authorize a transition or tool.
- A cancellation is a recorded runtime outcome. It is not represented as a
  successful assistant response.
- Secrets remain in the credential service; session entries carry references
  and redacted metadata, not raw provider keys.

## Failure and recovery

The runtime treats failure as a normal state transition:

- If a provider stream fails before completion, its partial text is not
  committed as the assistant's final answer.
- Context overflow may trigger bounded compaction and retry, but only after the
  compaction generation advances.
- Repeated identical tool failures stop the loop instead of retrying forever.
- If a state-changing tool process disappears before reporting an outcome, the
  result is `unknown`; it is not automatically replayed.
- A truncated final log record can be discarded during recovery. Corruption in
  an earlier record quarantines the affected session instead of silently
  skipping history.
- Finalization is repairable after restart, and only one terminal final is
  accepted for a turn.
- Reconnecting clients receive replayed events or a fresh snapshot so their
  projection can catch up.

## Current limitations and status

- The core actor, session-store, branching, action-queue, cancellation, and
  restart paths have focused test coverage in the repository.
- Durable prompt acceptance before lazy worker activation is still an open
  integration item. If the transport fails during a cold start, before the
  client observes authoritative acceptance, the client may not know whether a
  prompt was durably admitted. Do not automatically resend a state-changing
  request in that case.
- A live streaming response is intentionally provisional until its terminal
  event is committed.
- Session durability does not make external providers or tools transactional.
  Their effects can remain unknown after a network or process failure.
- The active release-wide security and full-integration gates are broader than
  the focused session tests and remain separate qualification work.

## Validate changes

For changes in this area, validation should cover the owning crate and the real
daemon transport. Relevant cases include:

- the full legal and illegal session-state transition matrix;
- actor serialization and bounded-mailbox behavior under concurrent callers;
- branch reconstruction without history rewriting;
- checksums, truncation recovery, quarantine, leases, compaction, and archive;
- prompt, steering, cancellation, tool waiting, and finalization;
- daemon restart, worker loss, reconnect replay, and profile isolation;
- a real provider stream where partial output fails before completion.

Use disposable Cargo target directories as described in
[Contributing](../../CONTRIBUTING.md). Focused unit success is not a substitute
for a socket-level restart journey when protocol behavior changes.

## Key source locations

- `crates/session/src/lib.rs` — session identity, state machine, actor, and
  snapshots
- `crates/session-store/src/lib.rs` — append-only history, branches, leases,
  compaction, export, and recovery
- `crates/action-store/src/lib.rs` — durable input queue, priority, admission,
  and delivery points
- `crates/agent-loop/src/lib.rs` — turn loop, streaming, tools, cancellation,
  compaction, and final commits
- `crates/daemon-core/` — daemon-owned session orchestration and projections
- `crates/local-runtime/src/lib.rs` — local runtime assembly and built-in
  services
- `crates/protocol/src/lib.rs` — client commands, events, and session views
- `apps/agentd/` — process lifecycle, socket server, worker supervision, and
  integration tests
- `apps/agent-tui/` and `apps/agent-web/` — client session controls
- `spec/keith-agent/spec.kvx` — implementation requirements and current task
  status
