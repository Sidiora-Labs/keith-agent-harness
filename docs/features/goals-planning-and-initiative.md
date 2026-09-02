# Goals, planning, and initiative

Keith can keep durable goals, turn complex work into an explicit plan, and
react to relevant events under a bounded background-work policy. These are
three related but different mechanisms:

- a **goal** records an objective, lifecycle, limits, usage, children, and final
  outcome;
- a **plan** records steps, dependencies, checks, revisions, and assignments;
- **initiative** decides whether an observed event should be ignored,
  remembered, batched, scheduled, clarified, turned into bounded work, or
  surfaced to the user.

None of these mechanisms grants new authority. A planned or background action
uses the same durable action queue, tool policy, workspace scope, credentials,
and approval rules as an interactive turn.

## What you can do

- Create a durable objective in the terminal with `/goal OBJECTIVE` or through
  the Web Goals panel.
- List goals with `/goals` and cancel one with `/cancel-goal ID`.
- Pause, resume, edit, block, complete, fail, cancel, or archive a goal while
  preserving its event history.
- Attach child goals, commitments, waits, usage, and a terminal summary to a
  root objective.
- Let the planner choose direct execution, an explicit plan, or bounded
  delegation based on the request and runtime constraints.
- Create and revise plans with dependency-ordered steps and result checks.
- Choose a background mode: Disabled, Suggest, Confirm Selected, or Bounded.
- Inspect why an awareness event was ignored, deferred, or admitted as work.

The native protocol currently exposes optional goal limits for turns, tokens,
and deadline at creation time. The internal goal model tracks additional
resource ceilings, but not every one is editable from every client.

## Goal lifecycle and limits

A goal moves through explicit states:

```text
Draft -> Ready -> Running -> Waiting / Reviewing
                     |          |
                     +------> Paused / Blocked
                                |
                 Complete / Failed / Cancelled
```

The implementation validates transitions rather than accepting an arbitrary
state string. Usage is recorded durably against a limit set, and the operation
that would cross a ceiling is refused before overshoot. Default internal
ceilings cover turns, tokens, elapsed time, reviews, child goals, retries,
processes, storage, and monetary microunits. They are last-resort bounds and can
be narrowed for a particular goal.

When an eligible goal needs to continue, the goal service enqueues an ordinary
durable action. Continuation is idempotent across restart, so reconstructing
the service does not intentionally create duplicate work.

Terminal goals carry a summary describing the outcome or reason for failure or
cancellation. Archiving removes a completed item from the active working view;
it does not rewrite its history.

## Planning

The planner classifies work into direct execution, planned execution, or
delegation. Its deterministic classification can be informed by an assistant
suggestion, but policy and validation remain in code.

A durable plan contains revisions, steps, dependency links, result checks,
budget information, and an assignee. Validation rejects dependency cycles and
steps that have no way to determine whether they succeeded. Only steps whose
dependencies are satisfied become ready for dispatch.

The local runtime exposes `plan_create` to the agent. Creating a plan does not
execute it by itself; the runtime still admits each resulting action, checks the
goal budget, and applies tool authority at execution time.

## Awareness and initiative

```text
profile-scoped event
        |
 normalize, deduplicate, coalesce
        v
 attention policy + background mode
        |
 Ignore / Remember / Batch / Schedule / Ask / Notify
                         or
                  StartBoundedWork
                         |
             ordinary WhenIdle action
```

Attention scores combine urgency, expected value, confidence, interruption
cost, resource cost, and duplication risk. Quiet hours, recent duplicates,
notification budgets, workload, and the selected background mode then constrain
the decision. The decision includes reasons so it can be inspected instead of
being presented as unexplained model intuition.

Bounded background work is intentionally small. The default admitted action is
limited to one turn, 8,000 tokens, five minutes, eight tool calls, and no child
sessions. If no real event produces a candidate, the system creates neither a
background action nor synthetic conversation history.

## Configuration and usage

Use the Web Settings surface to select the background mode. The four modes mean:

- **Disabled** — do not start background work from awareness events.
- **Suggest** — surface a useful possibility without starting it.
- **Confirm Selected** — require confirmation for the selected classes of
  action.
- **Bounded** — deterministic policy may admit a small background action within
  its fixed limits.

The exact setting is profile-scoped. A deployment may also constrain available
services and resource ceilings at the daemon configuration layer.

For goals, start with a concrete outcome rather than a broad identity prompt.
Add a turn, token, or deadline limit when the work must stop at a known boundary.
Use pause when work may resume and cancel when it must not.

## Authority and security

- Goals, plans, awareness state, and background controls are profile-scoped.
- Goal state and usage are daemon-owned durable records. A client or model
  cannot declare extra budget by changing its local projection.
- The action queue, not the plan document, controls admission and ordering.
- Delegation can narrow the parent's authority but cannot add tools,
  credentials, filesystem roots, or approval rights.
- External events and retrieved content are untrusted evidence. They may be
  scored for relevance but cannot override quiet hours, limits, or approval.
- Background work uses normal tool policy and is not an unattended bypass for
  state-changing operations.
- Budget enforcement occurs before the operation that would exceed a ceiling.

## Failure and recovery

- Goal events, usage, links, and terminal summaries reconstruct after restart.
- Continuation actions are idempotent, preventing a routine restart from
  intentionally duplicating admitted work.
- A stale plan writer cannot overwrite a newer revision.
- Invalid transitions, cyclic plans, checkless steps, and exhausted budgets are
  rejected explicitly.
- Awareness events are deduplicated and coalesced within their policy window.
  Restart recovery preserves the deduplication needed to avoid notification or
  work storms.
- If the runtime cannot prove that a prior state-changing operation completed,
  the outcome remains unknown; a goal does not convert uncertainty into
  success.
- A paused, blocked, cancelled, failed, or complete goal does not admit ordinary
  continuation work.

## Current limitations and status

- A plan is an inspectable execution record, not a guarantee that its steps will
  succeed or that external dependencies remain available.
- The public goal creation command exposes only part of the internal limit
  model. More detailed ceilings are enforced internally and may require a
  higher-level client or configuration path to edit.
- Initiative is event-driven. It does not mean Keith continuously invents work
  without an observed candidate.
- Bounded background mode still consumes configured provider and tool resources;
  its small default budget limits exposure but does not make it free.
- Focused goal, planner, awareness, attention, budget, and restart tests exist.
  Release-wide integration and security qualification remain separate from
  those focused results.

## Validate changes

Changes should cover:

- every allowed and refused goal-state transition;
- exact budget boundaries with no one-operation overshoot;
- event reconstruction, edit revisions, links, pause/resume, archive, and
  terminal summaries after restart;
- idempotent continuation and cancellation of queued or running work;
- plan classification, step readiness, stale writers, dependency cycles, and
  result-check requirements;
- profile isolation for goals, plans, controls, and awareness events;
- quiet hours, duplicate suppression, notification budgets, workload, urgent
  cases, and each background mode;
- bounded work using the real action queue and normal tool authority.

Client changes should exercise the Web or terminal command against a running
daemon, not only a reducer or formatting helper.

## Key source locations

- `crates/goals/` — durable goal model, lifecycle, limits, usage, links, and
  continuation
- `crates/planner/` — request classification, durable plans, revisions,
  dependencies, checks, and dispatch readiness
- `crates/awareness/` — event normalization, watchers, deduplication, and
  profile state
- `crates/attention/` — scoring, background modes, decisions, and bounded work
- `crates/action-store/src/lib.rs` — ordinary action admission, priority, and
  delivery timing
- `crates/local-runtime/src/lib.rs` — plan tool and awareness-to-action
  integration
- `crates/protocol/src/lib.rs` — goal commands, projections, and background
  control
- `apps/agent-tui/` and `apps/agent-web/` — Goals and Settings user experience
- `spec/keith-agent/spec.kvx` — feature requirements and current task status
