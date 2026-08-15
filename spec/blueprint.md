# Unified Rust Agent — Complete Product Blueprint

## Document control

| Field | Value |
|---|---|
| Status | Architecture baseline |
| Intended audience | Product, runtime, infrastructure, security, client, and integration teams |
| Implementation language | Rust for all owned platform components |
| Runtime foundation | Delta-1 |
| Assistant and distribution foundation | Gamma-3 |
| Companion document | [shematics.md](./shematics.md) |

## 1. Executive summary

The product is a persistent personal and professional agent built around a daemon-supervised recursive runtime.

Delta-1 supplies the core identity of the system:

- A stable daemon separate from clients.
- One worker per root session tree.
- A full AgentSession abstraction for both root and child agents.
- One action-admission path for prompts, schedules, messages, steering, and autonomous continuation.
- Provider-neutral model execution.
- Persistent computation.
- Append-only branched conversations.
- Crash recovery, reconnects, compaction, goals, and bounded autonomy.

Gamma-3 supplies the assistant product layer:

- Broad messaging-channel reach.
- Explicit profile and workspace routing.
- Human-readable persona, rules, memory, knowledge, and skills.
- Hybrid keyword, trigram, and vector retrieval.
- Scheduled and proactive delivery.
- Web and desktop access.
- Guarded, reversible background refinement of declarative state.

The combined system adds a persistent life loop:

```text
observe
  → update current state
  → select what deserves attention
  → maintain goals and commitments
  → plan a useful next action
  → execute through AgentSession
  → review the result
  → update memory and reusable skills
  → wait efficiently for the next event
```

The agent should feel alive because it maintains continuity, initiates useful work, remembers promises, notices meaningful changes, and returns with results. It should not simulate consciousness or fabricate internal activity.

## 2. Product definition

### 2.1 One-sentence definition

> A Rust-native, daemon-supervised recursive agent runtime with persistent computation, human-readable personal memory, proactive multi-channel delivery, and bounded long-running autonomy.

### 2.2 Product promise

The agent should be able to:

1. Continue a conversation or task across client disconnects and process restarts.
2. Work recursively by delegating bounded objectives to durable child sessions.
3. Operate through terminal, web, desktop, API, and messaging channels without duplicating its core behavior.
4. Remember user preferences and project context in files the user can inspect and edit.
5. Maintain goals, commitments, schedules, waiting conditions, and unfinished work.
6. Use tools and persistent compute without giving guest code unrestricted authority over the host.
7. Improve its declarative workflows and memory through reversible background review.
8. Choose when to answer, ask, wait, schedule, delegate, act, or stop.

### 2.3 Product truth

The system is not a new foundation model. Its intelligence comes from model selection, context quality, tool use, recursive decomposition, persistent state, review, and accumulated procedural knowledge.

Self-improvement means editing readable memory, instructions, preferences, knowledge, and skills. It does not mean modifying model weights or silently rewriting trusted runtime code.

Autonomy means durable, event-driven work within user-configured limits. It does not mean unrestricted access or infinite self-continuation.

## 3. Source lineage and ownership

| System area | Architectural owner | Reason |
|---|---|---|
| Daemon, workers, sessions, recursion | Delta-1 | Strongest process model and uniform recursive session abstraction |
| Action admission and continuation | Delta-1 | Keeps all work on one predictable execution path |
| Model/provider layer | Delta-1 | Clean provider normalization and streaming lifecycle |
| Persistent compute | Delta-1 | Session-scoped computational continuity |
| Branched history and compaction | Delta-1 | Natural replay, branching, resume, and context management |
| Goals and long-running control | Delta-1 | Durable limits, continuation, schedules, and child coordination |
| Messaging channels | Gamma-3 | Broad distribution and practical channel behavior |
| Profiles and workspaces | Gamma-3 | Clear personal-assistant identity and state separation |
| Memory and retrieval | Gamma-3 | Readable files with robust lexical/vector retrieval and fallback |
| Knowledge and skills | Gamma-3 | User-owned Markdown resources that can be reloaded dynamically |
| Proactive delivery | Gamma-3 | Returns scheduled and background work where the user already communicates |
| Background refinement | Gamma-3 | Restricted writes, snapshots, validation, rollback, and useful notifications |

## 4. Design principles

### 4.1 One runtime, many surfaces

The TUI, web application, desktop application, API clients, and messaging adapters all connect to the same daemon protocol. No client implements its own agent loop, memory rules, scheduler, or routing semantics.

### 4.2 One session abstraction

A parent, a retained child, a scheduled session, and a resumed session all use AgentSession. Stateless helper calls are allowed for classification or review but are never presented as full agents.

### 4.3 One admission path

Interactive prompts, channel messages, scheduled jobs, child messages, follow-ups, steering, and autonomous continuations enter the same ordered action inbox.

### 4.4 Human-owned personal state

Persona, preferences, rules, memory, knowledge, and skills remain readable files. The agent may propose and apply authorized changes, but the user can always inspect, edit, export, diff, and undo them.

### 4.5 Durable state over long-lived processes

Workers, kernels, browsers, and channel connections may disappear. Sessions, goals, commitments, schedules, messages, artifacts, and deliveries must survive.

### 4.6 Event-driven autonomy

The agent wakes for meaningful events instead of spinning in an infinite reasoning loop. Waiting is a durable state, not an active process consuming tokens.

### 4.7 Useful initiative over noise

The agent may generate many possible initiatives but must rank, suppress, batch, schedule, or discard them according to urgency, expected value, interruption cost, quiet hours, and user preferences.

### 4.8 Real progress over performance theater

Progress messages, presence indicators, and proactive notifications must correspond to actual session, child, kernel, scheduler, channel, or tool state.

### 4.9 Reversible change by default

Workspace edits, generated skills, memory consolidation, configuration changes, and external drafts should be staged or snapshotted whenever practical.

### 4.10 Explicit limits

Every long-running goal, child, schedule, tool, kernel, and autonomous continuation has time, token, retry, concurrency, and resource limits.

## 5. User groups

### 5.1 Individual operator

Uses the agent locally for coding, research, writing, planning, reminders, knowledge management, and personal automation.

Needs:

- Strong privacy and local ownership.
- Simple installation.
- Readable memory.
- Predictable confirmations for dangerous actions.
- Good terminal and desktop experiences.

### 5.2 Technical power user

Uses recursive agents, persistent kernels, projects, MCP servers, custom models, extensions, and scheduled workflows.

Needs:

- Inspectable session trees.
- Fine-grained model and tool configuration.
- Artifact access.
- Stable RPC and extension interfaces.
- Reproducible execution.

### 5.3 Always-on assistant user

Interacts mainly through messaging channels and expects reminders, summaries, monitoring, and proactive follow-up.

Needs:

- Reliable routing.
- Quiet hours and notification budgets.
- Durable schedules.
- Clear separation among profiles and workspaces.
- Visible delivery failure and retry state.

### 5.4 Team operator

Runs several profiles or shared assistants for projects or groups.

Needs:

- Explicit identity mapping.
- Workspace isolation.
- Shared versus private memory boundaries.
- Per-profile models and credentials.
- Operational dashboards and quotas.

## 6. Product modes

| Mode | Purpose | Runtime behavior |
|---|---|---|
| Interactive TUI | Deep local work | Attach to daemon, stream all events, expose sessions, branches, children, goals, and tools |
| One-shot | Scripts and shell composition | Submit one action, wait for final output, use stable exit codes |
| JSON/RPC | Programmatic control | Framed commands and ordered events |
| Web | Rich multi-session operation | Reconnectable streaming, memory editing, schedules, knowledge, channels, and artifacts |
| Desktop | Packaged personal assistant | Starts daemon, provides notifications, file integration, and local settings |
| Channel | Everyday conversation | Route external identity to a profile/session and deliver replies asynchronously |
| Background | Scheduled, triggered, or idle work | Enqueue ordinary session actions and release resources while waiting |
| Embedded | Host application integration | Use the same connection contract against an in-process or managed daemon endpoint |

## 7. A–Z capability blueprint

### A — Action admission

All work enters a durable, ordered SessionAction inbox. Each action has a source, delivery timing, priority, limits, reply route, and cancellation identity.

### B — Background work

Scheduled tasks, idle reviews, file watches, child completion, and external triggers wake sessions without creating alternate agent loops.

### C — Channels

Messaging adapters normalize external messages, preserve per-conversation order, stage attachments, suppress duplicates, and deliver responses through a durable outbox.

### D — Daemon

A stable Rust supervisor owns session discovery, worker lifecycle, attachments, reconnect cursors, schedules, routing, and global resource limits.

### E — Evolution

A restricted reviewer may propose changes to user-owned declarative state. Changes are path-confined, snapshotted, validated, diffed, and reversible.

### F — Files and artifacts

Workspace tools operate from confined roots. Large outputs, generated documents, kernel snapshots, and child deliverables live in session artifact directories.

### G — Goals

Durable goals carry an objective, limits, status, current plan, waiting state, and completion summary. Autonomous continuation is attached to a goal, never to an unbounded loop.

### H — History

Conversation history is append-only and branched. Context reconstruction follows a selected leaf and the latest compatible compaction boundary.

### I — Identity

External users, channels, agent profiles, workspaces, and sessions map explicitly. The router never silently substitutes a different profile.

### J — Jobs

Persistent jobs support one-time, interval, and calendar schedules, time zones, missed-run behavior, delivery routes, retries, pause, resume, and history.

### K — Knowledge

Markdown pages and relative links form a lightweight personal wiki with backlinks, search, safe rename, orphan detection, and related-page discovery.

### L — Life loop

Awareness, current state, attention, commitments, initiative, review, and efficient waiting make the agent temporally continuous between conversations.

### M — Memory

Persona, preferences, durable memory, daily notes, and session summaries remain distinct. Retrieval combines keyword, trigram, and optional vector search.

### N — Network integrations

Web access, model providers, MCP servers, channels, and plugins use explicit connection configuration, timeouts, destination restrictions, and isolated credentials.

### O — Observability

Operators can inspect worker state, session events, tool calls, children, kernels, schedules, deliveries, usage, errors, and background actions.

### P — Planning and providers

Simple requests use direct execution. Complex goals receive milestones, next actions, dependency ordering, and review. Provider routing selects models by profile, task, latency, context, and budget.

### Q — Quality review

Important results receive a separate review pass using the original request and produced artifacts. Deterministic tests and validators are preferred over model judgment.

### R — Recursive agents

Durable children are full AgentSessions with independent history, goals, artifacts, budgets, communication, cancellation, and retention.

### S — Skills and sandboxing

Skills are declarative workflows selected into context. Executable tools and guest runtimes operate in restricted processes, containers, or WASI environments.

### T — Tools

Tools have typed schemas, timeouts, output limits, concurrency behavior, and execution isolation. The loop handles malformed calls, repeated failures, cancellation, and oversized output.

### U — User control

The user controls autonomy mode, quiet hours, models, budgets, workspaces, channels, memory, schedules, dangerous-action confirmation, and background refinement.

### V — Vector and lexical retrieval

Vector search improves semantic recall but remains optional. Keyword and trigram retrieval work without embeddings, and all indexes are rebuildable.

### W — Workspaces and waiting

Profiles own explicit workspaces. Waiting conditions release compute and resume on time, process, child, channel, file, or external events.

### X — Extensions

First-party extensions are compiled Rust crates. Third-party executable extensions use versioned WASI interfaces or separate processes. MCP remains a separate integration protocol.

### Y — Yield and resource control

Sessions yield while waiting. Daemon, worker, provider, child, kernel, channel, and scheduler concurrency are independently bounded.

### Z — Zero-loss recovery goal

Committed prompts, session entries, goals, schedules, workspace updates, and outbound deliveries survive process failure. Uncommitted in-flight operations resolve to an explicit interrupted state.

## 8. High-level subsystem model

```text
CLIENT PLANE
  CLI · TUI · Web · Desktop · RPC · Messaging adapters
                         │
                         ▼
CONNECTION PLANE
  AgentConnection protocol · attachments · reconnect · ordered events
                         │
                         ▼
SUPERVISION PLANE
  daemon · session catalog · worker leases · scheduler · routing · delivery
                         │
                         ▼
SESSION PLANE
  AgentSession · action inbox · goals · plans · context · compaction · children
                         │
             ┌───────────┴───────────┐
             ▼                       ▼
MODEL PLANE                    EXECUTION PLANE
  providers · streaming        tools · kernel · browser
  routing · retries            files · shell · MCP · plugins
             │                       │
             └───────────┬───────────┘
                         ▼
PERSONAL-STATE PLANE
  profile · memory · daily notes · knowledge · skills · current state
                         │
                         ▼
BACKGROUND PLANE
  awareness · attention · commitments · initiative · evolution · delivery
                         │
                         ▼
PERSISTENCE PLANE
  session logs · state DB · Markdown workspace · indexes · artifacts · secrets
```

## 9. The life loop

### 9.1 Purpose

The life loop gives the agent continuity and initiative without inventing emotions. It is a daemon-level service that reacts to real events and enqueues ordinary AgentSession actions when useful.

### 9.2 Inputs

- New user or channel message.
- Time and schedule events.
- File and repository changes.
- Child completion or failure.
- Process, build, or deployment completion.
- External connector event.
- Commitment deadline.
- Goal inactivity.
- Session idle transition.
- Explicit user feedback.

### 9.3 Persistent current state

The workspace contains compact current-state projections:

```text
state/
  now.md
  projects/
  relationships/
  commitments.toml
  routines.toml
  waiting.toml
  feedback.toml
```

Current state records active facts such as focus, open projects, recent interactions, waiting items, and upcoming commitments. It is not a complete transcript and is kept small enough for selective prompt inclusion.

### 9.4 Attention

Every event may generate zero or more initiative candidates. Candidates are ranked by:

- Urgency.
- User-declared importance.
- Deadline distance.
- Change since last observation.
- Likelihood that action will help.
- Cost and risk.
- Interruption cost.
- Quiet hours.
- Recent duplicate notifications.
- Current workload and budget.

The attention engine selects one of:

- Ignore.
- Remember without notifying.
- Batch into a later digest.
- Schedule a check.
- Ask the user.
- Start bounded work.
- Notify immediately.

### 9.5 Commitments

The agent extracts or explicitly creates promises and obligations. Every commitment has an owner, due or trigger condition, status, related session, and response route.

Commitments transition through:

```text
captured → scheduled → active → waiting → fulfilled
                         ├── blocked
                         ├── cancelled
                         └── expired
```

The user can list, edit, pause, cancel, or complete commitments from any full client.

### 9.6 Presence

Presence is a projection of real activity:

- Available.
- Thinking.
- Using tools.
- Waiting for a child.
- Waiting for an external condition.
- Paused for user input.
- Scheduled to resume.
- Completed.
- Failed.

Channel-friendly progress updates are rate-limited and only emitted after meaningful state transitions.

## 10. Intelligence architecture

### 10.1 Direct versus planned execution

The router classifies work into:

- Direct answer.
- Single-tool action.
- Multi-step task.
- Research task.
- Coding/project task.
- Monitoring task.
- Scheduled recurring task.
- Delegated task.

Direct tasks avoid planning overhead. Complex tasks receive a lightweight plan.

### 10.2 Plan structure

A plan contains:

- Restated outcome.
- Constraints.
- Milestones.
- Dependencies.
- Next executable actions.
- Assigned root or child session.
- Result checks.
- Budget allocation.
- Revision history.

Plans are mutable working state. The user may inspect or edit them, and execution may revise them after failures or discoveries.

### 10.3 Review

Review is separate from execution for important work. It receives:

- Original request.
- Current plan.
- Final response.
- Relevant artifacts and tool outputs.
- Deterministic test results.

Review returns accept, revise, ask-user, or stop. The system does not endlessly alternate executor and reviewer; each goal has a bounded review count.

### 10.4 Model routing

Profiles define default and fallback models. The router may select specialized models for:

- Fast classification.
- General conversation.
- Complex reasoning.
- Coding.
- Vision.
- Summarization.
- Review.

Selection considers capability, latency, price, context size, tool support, recent reliability, and remaining goal budget. Users can pin a model at session or action scope.

### 10.5 Tool experience

The system records operational statistics, not hidden reasoning:

- Success and failure categories.
- Average latency.
- Timeout frequency.
- Common corrective actions.
- Task categories where a tool works well.
- Provider-specific incompatibilities.

The router uses these statistics to avoid repeatedly failing approaches and select proven tools or skills.

### 10.6 Skill learning

Successful repeated workflows may produce candidate skills. A candidate includes trigger conditions, required inputs, steps, validation, known failures, and stop conditions.

Candidates are tested in a clean workspace and installed only after policy and user preferences allow it. Installed skills remain readable and editable.

## 11. Autonomy architecture

### 11.1 Autonomy modes

| Mode | Allowed behavior |
|---|---|
| Observe | Monitor and summarize only |
| Suggest | Prepare plans, drafts, and patches without applying them |
| Act locally | Modify approved workspaces and run configured local tools |
| Operate | Perform pre-authorized external operations and deliveries |
| Confirm selected actions | Mix automatic low-impact work with explicit confirmation categories |

### 11.2 Limits

Each profile and goal may limit:

- Tokens and monetary spend.
- Elapsed time.
- Model turns.
- Tool calls.
- Concurrent children.
- Recursive depth.
- Background actions per hour/day.
- Notifications per channel.
- Retries per failure category.
- Kernel CPU, memory, and lifetime.
- Filesystem roots.
- Network destinations.

The most restrictive applicable setting wins.

### 11.3 Durable goal lifecycle

```text
draft
  → ready
  → running
  → waiting
  → running
  → reviewing
  → complete

Any active state may also transition to paused, blocked, failed, or cancelled.
```

Waiting conditions include time, message, child, process, file change, repository change, network condition, or user response.

### 11.4 Stagnation detection

Autonomous work stops or changes strategy when it detects:

- Repeated identical tool failures.
- No plan progress across multiple turns.
- Repeated edits and reversions of the same content.
- Multiple children returning equivalent failures.
- No new artifacts or state changes despite continued token use.
- An impossible or expired waiting condition.
- Exhausted time, token, retry, or resource limits.

Responses are change-strategy, narrow-scope, ask-user, wait, or stop.

### 11.5 Reversibility

The agent should:

- Create a branch or snapshot before material workspace edits.
- Produce message drafts before external send when configured.
- Stage destructive operations.
- Keep prior versions of self-evolved files.
- Record external message identifiers.
- Support one-step undo for declarative state changes.

## 12. Session and recursion model

### 12.1 Root tree

A root session is the ownership boundary for a worker. All branches, retained children, schedules, goals, kernels, and artifacts associated with that tree are discoverable beneath it.

### 12.2 Child types

| Child type | Use | Persistence |
|---|---|---|
| Durable child | Long-running research, implementation, monitoring, or specialist work | Full AgentSession history and artifacts |
| Isolated child | Risky or resource-heavy work | Full session plus separate executor boundary |
| Stateless helper | Classification, ranking, summarization, or bounded review | Request/response record only |

### 12.3 Child contract

Every durable child receives:

- Objective.
- Relevant context.
- Deliverable format.
- Workspace mode.
- Tool and model configuration.
- Token, turn, time, and process limits.
- Messaging rate limit.
- Retention policy.

The parent remains responsible for integration and final user communication.

### 12.4 Messaging

Parent and child communication is typed, durable, and rate-limited. Messages may carry text, status, requests, or artifact references. Children cannot recursively broadcast across unrelated session trees.

## 13. Memory, knowledge, and context

### 13.1 Memory classes

| Class | Purpose | Storage |
|---|---|---|
| Session history | Exact conversational and lifecycle record | Branched append-only log |
| Compaction summary | Compressed context for one branch | Session entry |
| Daily memory | Chronological personal activity | `memory/YYYY-MM-DD.md` |
| Durable memory | Stable preferences, facts, and recurring context | `MEMORY.md` |
| Current state | Active projects, commitments, waiting, and focus | `state/` files |
| Knowledge | User-curated or agent-maintained reference pages | `knowledge/` Markdown |
| Skills | Reusable procedures | `skills/` packages |
| Artifacts | Generated files and large tool outputs | Session artifact directory |

### 13.2 Retrieval policy

Context retrieval follows this order:

1. Exact current session branch.
2. Active goal, plan, commitments, and waiting state.
3. Profile and user rules.
4. Relevant current-state records.
5. Relevant durable and daily memory.
6. Relevant knowledge pages.
7. Relevant skills.
8. Child status and results.

Retrieval respects profile and workspace boundaries. Each result retains its source path and section so users can inspect it.

### 13.3 Memory writes

Memory writes may be:

- Explicitly requested by the user.
- Proposed during conversation.
- Generated during compaction.
- Produced by scheduled consolidation.
- Proposed by background refinement.

Stable preference updates may apply automatically when permitted. Sensitive identity, relationship, or rule changes should be presented as a diff or require confirmation according to profile settings.

### 13.4 Forgetting and cleanup

The user can remove or correct memory directly. The system also supports:

- Expiring temporary state.
- Archiving old daily notes.
- Detecting duplicated memory.
- Marking superseded preferences.
- Rebuilding indexes after deletion.
- Removing embeddings and cached summaries associated with deleted source text.

## 14. Channels and routing

### 14.1 Adapter contract

Every channel implements inbound normalization, outbound delivery, identity mapping, attachments, retry classification, rate limits, and supported interaction features.

### 14.2 Ordering

Messages within the same external conversation are delivered to one session queue in order. Separate conversations may execute concurrently subject to global limits.

### 14.3 Routing

Routing keys may include channel, external account, group, thread, sender, command prefix, or explicit profile selection.

Routing resolves:

- Profile.
- Workspace.
- Existing or new session.
- Model configuration.
- Reply route.
- Group/private memory policy.

No fallback may expose another profile's workspace or memory.

### 14.4 Group conversations

Group profiles need explicit rules for:

- When the agent should respond.
- Mention requirements.
- Shared versus private memory.
- Whether participant preferences may be retained.
- Who can create schedules or invoke tools.
- Where proactive messages may be posted.

### 14.5 Delivery behavior

The outbox supports exactly-once intent with idempotent platform keys where possible. Because external platforms cannot always guarantee exactly-once delivery, duplicates are detected and exposed rather than silently hidden.

## 15. Tool and compute model

### 15.1 Tool categories

- Read-only workspace tools.
- Workspace mutation tools.
- Shell and build tools.
- Web search and fetch.
- Browser automation.
- Memory and knowledge tools.
- Scheduler tools.
- Child-agent tools.
- MCP tools.
- Media and artifact tools.
- External communication tools.

### 15.2 Tool requirements

Every tool defines:

- Stable name and version.
- Input and output schema.
- Timeout and output limit.
- Read/write/network/process behavior.
- Whether parallel execution is safe.
- Retry behavior.
- Cancellation behavior.
- Required configuration or credential reference.
- User-facing activity label.

### 15.3 Execution boundaries

| Workload | Default boundary |
|---|---|
| Pure transformation | In-process Rust with strict input limits |
| Third-party plugin | WASI component |
| Filesystem operation | Workspace-rooted file handles |
| Shell, build, or compiler | Restricted child process |
| Untrusted project | Container or stronger sandbox |
| Browser | Isolated automation service and profile |
| Guest notebook | Dedicated sandboxed kernel process |
| Host desktop action | Narrow user-confirmed operation |

### 15.4 Persistent compute

The kernel broker maintains one or more session-scoped compute environments. It supports startup, execution, interrupt, output streaming, snapshots, restore, idle eviction, and resource limits.

The broker is Rust. Guest kernels may run Python or other languages. Guest-to-agent operations use typed bridge messages and the same tool system as model-originated calls.

## 16. Extensions

### 16.1 Skills

Readable instructions and resources selected into context. They cannot execute solely by being loaded.

### 16.2 Plugins

Executable extensions with versioned interfaces. First-party native plugins compile into the product. Third-party plugins use WASI or separate processes without ambient host access.

### 16.3 MCP

MCP configuration, authentication, connection lifecycle, schema caching, and relevance selection are daemon services. Sessions receive only enabled tool projections for their profile.

### 16.4 Extension lifecycle

Extensions support discovery, validation, activation, health, disable, update, migration, and uninstall. A failing extension cannot prevent the daemon from starting in safe mode.

## 17. User experience

### 17.1 Core conversation

Every full client displays:

- Streaming assistant text.
- Tool and child activity.
- Stop, steer, retry, and branch controls.
- Current model and usage.
- Goal and waiting status.
- Attachments and artifacts.

### 17.2 Session navigation

Users can list, search, label, branch, resume, archive, export, and delete sessions. Branch selection is explicit and visible.

### 17.3 Memory experience

Users can:

- Open source Markdown.
- Search all memory and knowledge.
- See which profile owns a memory.
- Edit or delete it.
- Rebuild indexes.
- Review consolidation and evolution diffs.
- Undo agent-generated changes.

### 17.4 Autonomy experience

Users can inspect:

- Active and paused goals.
- Commitments.
- Waiting conditions.
- Schedules.
- Proactive candidate history.
- Notification limits.
- Background resource usage.

Every autonomous item has pause, cancel, resume, and open-session actions.

### 17.5 Presence

Presence communicates actual state using concise labels and timestamps. The agent should acknowledge long-running work quickly, then send updates only for meaningful progress, waiting, failure, or completion.

## 18. Configuration and profiles

Configuration layers:

1. Built-in defaults.
2. Global user configuration.
3. Profile configuration.
4. Workspace/project configuration.
5. Session overrides.
6. One-action overrides.

More specific layers override broader layers, except mandatory safety ceilings that cannot be relaxed at narrower scope.

Profiles configure identity, model routing, workspace, memory, tools, skills, MCP, channels, schedules, autonomy, notifications, and evolution.

Configuration changes are validated before activation. Invalid project configuration cannot corrupt the global profile; the previous valid configuration remains active with a visible warning.

## 19. Safety, privacy, and user control

### 19.1 Default trust model

The local user owns the daemon and data. Models, websites, repositories, channel participants, MCP servers, plugins, and guest code are not automatically trusted.

### 19.2 Default protections

- Workspace confinement on by default.
- SSRF and local-network protection on by default.
- Minimal subprocess environment.
- Credentials excluded from prompts, logs, artifacts, and broad child environments.
- Third-party extensions isolated.
- External sends and destructive actions confirmable by category.
- Per-profile memory separation.
- Quiet hours and proactive-notification limits.
- Clear stop and kill controls.

### 19.3 Autonomy controls

Users can define allow, ask, or deny behavior for filesystem writes, shell execution, browser mutations, external messages, purchases, account changes, scheduling, and self-evolution.

### 19.4 Data lifecycle

Users can export or delete sessions, workspaces, artifacts, indexes, schedules, routes, and credentials separately. Derived indexes must be purged when their source data is deleted.

## 20. Persistence and ownership

| State | Source of truth | Recovery behavior |
|---|---|---|
| Session conversation | Append-only branched log | Reconstruct selected branch and settings |
| Worker ownership | Transactional state store plus lease | Expired owner can be replaced |
| Goal and plan | Session-scoped durable records | Resume running/waiting state |
| Commitments and waiting | Profile/workspace state | Wake on due trigger after restart |
| Schedules | Transactional scheduler store | Claim according to missed-run policy |
| Deliveries | Transactional outbox | Retry or expose terminal failure |
| Personal memory | Markdown workspace | Reload files and rebuild indexes |
| Retrieval | Derived indexes | Quarantine and rebuild |
| Artifacts | Session artifact directory | Reattach by stable reference |
| Kernel snapshots | Session artifact directory | Restore into compatible sandbox |
| Credentials | OS or restricted encrypted secret store | Reauthorize when unavailable |

## 21. Failure and recovery behavior

### 21.1 Client disconnect

The agent continues according to the action's policy. The client reconnects with generation and sequence cursors and receives a current snapshot plus missed events.

### 21.2 Worker crash

The supervisor starts or adopts a replacement, claims the tree lease, reconstructs sessions, and marks uncertain in-flight operations interrupted. It does not blindly repeat non-repeatable external actions.

### 21.3 Provider failure

Retry according to classified failure, then use configured fallback or pause with a visible error. Partial assistant content remains marked incomplete.

### 21.4 Tool failure

Record structured failure, truncate oversized diagnostic output, detect repeated calls, try a different approach when allowed, or stop after bounded retries.

### 21.5 Kernel failure

Restart the guest, restore the latest compatible snapshot, and notify the session of variables or processes that could not be restored.

### 21.6 Index corruption

Quarantine the index, fall back to direct or lexical access, rebuild in the background, and keep source files untouched.

### 21.7 Channel failure

Retain the outbound item, classify retryability, back off, and expose delivery state in full clients.

### 21.8 Evolution failure

Restore the snapshot, leave original files unchanged, record the failed proposal, and avoid notifying unless user attention is required.

## 22. Core user journeys

### 22.1 Continue a coding project

1. User resumes a project session from the TUI.
2. The worker restores the branch, goal, children, and kernel metadata.
3. Relevant project memory and skills enter context.
4. The agent plans remaining milestones.
5. Children inspect independent subsystems.
6. The root integrates changes, runs tests, reviews output, and reports artifacts.

### 22.2 Proactive daily assistant

1. A morning schedule wakes the profile.
2. The agent loads commitments, routines, and current-state files.
3. It gathers configured inputs.
4. It creates a concise prioritized brief.
5. The outbox delivers it through the user's preferred channel.

### 22.3 Monitor a long-running operation

1. The agent starts or attaches to a process.
2. It creates a waiting condition and releases the active model turn.
3. A process event wakes the session.
4. The agent interprets the result and either continues or reports.

### 22.4 Learn a preference

1. User corrects response style or workflow.
2. The agent creates a small candidate update to USER.md or a relevant rule.
3. Depending on profile policy, it applies or presents the diff.
4. The next context build uses the updated preference.

### 22.5 Turn a successful process into a skill

1. A repeated workflow succeeds.
2. Background review identifies reusable steps.
3. A candidate skill is created in a temporary workspace.
4. The workflow is validated.
5. The user reviews or auto-accepts according to policy.
6. Future matching tasks can select the new skill.

### 22.6 Multi-channel continuity

1. User starts a task on desktop.
2. The desktop disconnects while work continues.
3. A child completes and the root finishes.
4. The result is delivered to the configured messaging thread.
5. The user later opens the same session tree in the TUI.

## 23. Non-functional requirements

### 23.1 Reliability

- No committed prompt disappears after process failure.
- One lease holder writes a root session tree.
- Scheduler and delivery state survive daemon restart.
- Session reconstruction is deterministic.
- Background tasks expose terminal or recoverable states.

### 23.2 Performance targets

- Local daemon readiness should feel immediate on a normal developer machine.
- Existing-session attachment should not require loading unrelated session trees.
- Message acknowledgement should occur before long model or tool work.
- Retrieval should degrade gracefully without embeddings.
- Idle workers and kernels should not consume unbounded memory.

Exact service-level targets are defined after an instrumented prototype establishes realistic baselines.

### 23.3 Portability

- Linux is the reference execution platform.
- macOS and Windows receive platform-specific process, path, credential, and desktop implementations.
- Core session and workspace formats remain portable.
- Unsupported sandbox features fail closed or display an explicit reduced-isolation warning.

### 23.4 Maintainability

- No central runtime module becomes the home for unrelated behavior.
- Protocol and persisted formats are versioned.
- Provider and channel adapters are independently testable.
- Derived indexes can be discarded.
- Extension failures are isolated.

### 23.5 Accessibility

- Keyboard-complete TUI and web operation.
- Screen-reader labels for web controls.
- Reduced-motion support.
- High-contrast themes.
- Text alternatives for visual tool output.

## 24. Testing strategy

### 24.1 Unit tests

- State transitions.
- Routing rules.
- Context selection.
- Memory merging.
- Retry classification.
- Schedule calculations.
- Path confinement.
- Channel normalization.
- Provider event normalization.

### 24.2 Property tests

- Session-tree reconstruction.
- No duplicate sequence numbers.
- Action ordering.
- Lease exclusivity.
- Configuration merge rules.
- Path normalization and workspace containment.
- Serialization round trips.

### 24.3 Process tests

- Daemon restart.
- Worker crash and adoption.
- Client reconnect.
- Kernel failure.
- Child cancellation.
- MCP failure.
- Channel reconnect.
- Scheduler catch-up.

### 24.4 Adversarial tests

- Prompt attempts to read credentials.
- Symlink and path traversal.
- SSRF and local-network targeting.
- Malicious tool output.
- Plugin resource escape.
- Cross-profile memory access.
- Duplicate external events.
- Notification storms.

### 24.5 Product tests

- Clean installation.
- First model connection.
- First messaging-channel connection.
- Long conversation through compaction.
- Recursive project task.
- Scheduled proactive result.
- Memory edit and reload.
- Self-evolution rollback.
- Export, delete, and restore.

## 25. Operational telemetry

Collect locally visible metrics for:

- Active workers and sessions.
- Queue depth and action wait time.
- Model latency, tokens, cost, and errors.
- Tool latency and failure categories.
- Child count, depth, and duration.
- Kernel resource usage.
- Retrieval latency and index health.
- Scheduler lag.
- Delivery retries.
- Background initiatives proposed, suppressed, executed, and notified.
- Evolution proposals, changes, validation failures, and rollbacks.

Telemetry export is opt-in. Secrets, full prompts, personal memory, and tool outputs are excluded unless the user explicitly enables diagnostic capture.

## 26. Implementation roadmap

### Phase 0 — Contracts and formats

- Protocol versioning.
- Session-entry format.
- Workspace layout.
- Configuration layering.
- Adapter and provider traits.
- Failure taxonomy.

### Phase 1 — Delta-1 core in Rust

- Daemon and connection protocol.
- Worker-per-root-tree supervision.
- AgentSession actor.
- Unified action inbox.
- Branched session logs and leases.
- Provider-neutral streaming loop.
- CLI and TUI.

### Phase 2 — Execution depth

- Files, shell, web, and browser tools.
- Restricted executors.
- Persistent kernel broker.
- Recursive children.
- Goals, continuation, messaging, heartbeats, and compaction.
- MCP and skills.

### Phase 3 — Gamma-3 personal state

- Profiles and workspace routing.
- Persona, user, rules, memory, and daily notes.
- Knowledge pages.
- Keyword and trigram retrieval.
- Optional embeddings and vector retrieval.
- File reload and conflict handling.

### Phase 4 — Channels and proactive work

- Channel gateway.
- First high-value messaging adapter.
- Durable scheduler.
- Commitments and waiting conditions.
- Outbound delivery queue.
- Web client and desktop packaging.

### Phase 5 — Life loop and smarter work

- Awareness and current state.
- Attention and initiative ranking.
- Planning and review.
- Model routing.
- Tool experience.
- Specialized child roles.

### Phase 6 — Refinement ecosystem

- Guarded self-evolution.
- Candidate skill synthesis.
- WASI plugin SDK.
- Relevant MCP schema selection.
- Additional channel adapters.
- Long-running stress and recovery validation.

## 27. MVP

The first credible release includes:

- Rust daemon and root-session worker.
- AgentConnection protocol.
- CLI and Ratatui TUI.
- Two model providers.
- Branched session persistence and correct compaction.
- Filesystem, restricted shell, web fetch, and browser tools.
- Persistent guest-kernel broker.
- One full recursive child path.
- Goals and bounded continuation.
- Profile, USER.md, AGENT.md, RULE.md, MEMORY.md, and daily memory.
- Keyword/trigram retrieval.
- Durable scheduler and waiting conditions.
- One messaging channel and outbound delivery queue.
- Basic commitments and current-state view.

Deferred from MVP:

- Large channel catalog.
- Full desktop polish.
- Automatic skill synthesis.
- Background self-evolution.
- Third-party plugin marketplace.
- Advanced vector retrieval.
- Broad unattended external operations.

## 28. Release gates

### Runtime gate

- Root and child sessions survive worker termination.
- Reconnect returns ordered missed events without duplicate prompts.
- Context continuation selects the correct branch.
- Resource limits reclaim idle workers and kernels.

### Assistant gate

- Profile routing never crosses workspaces.
- Direct file edits appear in future context.
- Retrieval functions without embeddings.
- Memory deletion removes associated index records.

### Autonomy gate

- Waiting releases active resources and resumes on the correct trigger.
- Goals stop at configured limits.
- Repeated failures produce strategy change or stop.
- Scheduled work survives restart.
- Proactive delivery respects quiet hours and notification budgets.

### Safety gate

- Workspace traversal and symlink escapes are blocked.
- Guest code cannot read daemon credentials.
- Network tools enforce destination restrictions.
- Third-party plugins cannot access undeclared resources.
- Background refinement cannot modify protected files.

### Product gate

- Clean installation succeeds on supported platforms.
- First-run provider setup is understandable.
- Users can export, inspect, edit, and delete their data.
- All background work is discoverable and cancellable.

## 29. Risks and mitigations

| Risk | Consequence | Mitigation |
|---|---|---|
| Excessive scope | Slow and unreliable release | Enforce MVP boundary and phase gates |
| AgentSession monolith returns | High change coupling | Split actor into domain state machines and crate-owned services |
| Too many background messages | User disables proactivity | Attention ranking, quiet hours, batching, and notification budgets |
| Memory becomes noisy | Worse answers and user distrust | Separate current state, daily logs, durable memory, and session summaries |
| Recursive resource explosion | Cost and system instability | Depth, child, token, time, kernel, and process limits |
| Guest execution escapes | Host compromise | Separate process/container boundaries, minimal mounts and environment |
| Channel identity collision | Cross-user state exposure | Stable external identity mapping and explicit route ownership |
| Self-evolution damages behavior | Persistent degradation | Restricted targets, snapshots, validation, diff, and undo |
| Provider fragmentation | Inconsistent behavior | Normalized event model and provider conformance suite |
| Recovery retries external action | Duplicate real-world effect | Operation-specific retry rules and explicit interrupted state |
| Pure-Rust goal conflicts with Python utility | Lost computational ecosystem | Rust-owned kernel broker with optional sandboxed guest runtimes |

## 30. Product invariants

1. Every source of work uses the AgentSession action inbox.
2. One worker lease holder owns a root session tree at a time.
3. Every durable child is a full AgentSession.
4. A client disconnect does not erase committed work.
5. Session branches and compaction reconstruct deterministically.
6. Waiting does not consume model tokens or require a live worker indefinitely.
7. Profile routing never silently falls back across workspaces.
8. Human-readable memory remains editable and exportable.
9. Derived search indexes are never the only copy of personal state.
10. Background refinement is confined, validated, visible, and reversible.
11. Progress messages describe real runtime state.
12. Proactive behavior respects quiet hours, relevance, and notification limits.
13. Recursive work has explicit depth, time, token, and process limits.
14. Guest execution does not inherit unrestricted host credentials or paths.
15. Every persistent background item can be inspected, paused, resumed, or cancelled.

## 31. Glossary

| Term | Definition |
|---|---|
| Action inbox | Ordered admission point for every kind of session work |
| AgentConnection | Transport-independent client contract for daemon operations and events |
| AgentSession | Durable state machine representing one conversational and operational agent |
| Artifact | File or large output associated with a session or child |
| Attention candidate | Possible proactive action awaiting ranking and suppression |
| Branch | Selected path through an append-only session tree |
| Commitment | Promise or obligation with an owner, trigger, and lifecycle |
| Compaction | Creation of a concise context boundary for a session branch |
| Current state | Compact representation of active focus, projects, relationships, and waiting items |
| Durable child | Full retained AgentSession created by another session |
| Evolution | Restricted background review and reversible update of declarative workspace state |
| Goal | Durable objective with limits, plan, status, and continuation behavior |
| Guest kernel | Sandboxed external computational runtime managed by the Rust broker |
| Initiative | Candidate proactive action generated from an event or current state |
| Profile | Identity, model, workspace, memory, tools, channels, and autonomy configuration |
| Reply route | Destination used to deliver the result of an action |
| Root session tree | Root session and every branch or child owned by one worker |
| Session action | Prompt, schedule, message, steering, follow-up, or continuation admitted to a session |
| Waiting condition | Durable trigger that resumes a goal without keeping active compute alive |
| Workspace | User-owned directory containing profile state, memory, knowledge, skills, and project files |

## 32. Final architecture statement

The system should retain Delta-1's disciplined recursive runtime and Gamma-3's practical assistant reach while correcting both systems' operational weaknesses through a modular Rust implementation.

Its sense of life comes from continuity, attention, commitments, initiative, and truthful presence. Its intelligence comes from context selection, planning, tool experience, specialized children, and review. Its autonomy comes from durable goals, event triggers, efficient waiting, resource limits, and reversible action.
