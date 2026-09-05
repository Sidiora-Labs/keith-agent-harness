# Causal intelligence source baseline

## Evidence scope

- Feature: `keith-causal-intelligence`; task: **1.1**.
- Checkout: `/root/agent-keith`.
- Inspected Git HEAD: `3c70d2c8bf4ea57b8f261b67bc11d67ee801102c`.
- Inspection date: 2026-09-05.
- Evidence level: **source inspection only**. Runtime baseline execution is pending
  separately; this document is not a passing runtime report or task 1.3 evidence.
- At inspection, tracked Rust source was unchanged. The working tree contained an
  edited `spec/workflow.kvx` and untracked research, qualification-script and new
  specification directories. Therefore HEAD identifies the inspected runtime
  baseline, not a clean-tree assertion about the entire feature workspace.
- Locations below refer to this source baseline. Later implementation may shift
  line numbers; the named symbols are the durable lookup anchors.

No model invocation, embedding request, daemon restart, packaged application,
browser journey, or external integration was exercised for this source map.
In particular, a source-level guard is not proof that every runtime path reaches
that guard.

Baseline build follow-up: the first current-source build of `keith-agentd`,
`keith-agent-worker`, `keith-agent-web`, and `keith-agent-cli` exited 101 with
E0433 at `apps/agent-web/src/server.rs:636`: the upload handler used `EntityId`
without importing it. The implementation session restored the missing import.
Focused strict Clippy then exposed existing upload-handler style violations;
the same handler received behavior-preserving let-else, if-let, and digest
formatting changes. Focused Web tests passed (18 executed; the existing live
Chromium journey remained ignored), and strict package Clippy passed. These
pre-existing repairs do not qualify semantic memory or browser behavior.

## Preserved runtime baseline failures

- The first isolated launch exceeded the 60-second socket readiness deadline
  with the original large debug binaries. Setup failed before any test ran.
  Later probes launch stripped copies of freshly built binaries and record both
  source-binary and launched-binary hashes; stripping does not modify the build.
- The first full runner invocation, `20260905T040216Z-75a1f53b6a2949449b732b908844d281`,
  passed 24 runner checks and the ingestion/paraphrase probe but failed the
  ordinary exact-format assertion. It also invalidated its source proof because
  the baseline rebuilt a changed Web binary after the runner's initial hashes.
  Neither outcome is a passing qualification.
- A subsequent ordinary-only diagnostic confirmed the format mismatch: the
  fresh profile's required first-meeting introduction preceded the correct
  numeral. `local-runtime::relationship_prompt` mandates that introduction.
  The baseline fixture now needs an explicit, recorded first-meeting exchange
  before measuring established-profile instruction following. The original
  first-meeting conflict is preserved here; it is not repaired by changing the
  expected answer or by modifying Keith's personality policy.
- Run `20260905T041248Z-d31d2f8666ba4382884a7819387a21b3` failed during setup
  before either test executed, despite three completed HTTP-200 provider
  requests. The old diagnostic captured no exception or terminal details, so
  the cause remains unknown. The fixture now records bounded setup/terminal
  failures and explicitly requests no tools during onboarding. A subsequent
  direct diagnostic passed both tests; that does not explain or erase the
  earlier setup failure.

The successful paraphrase observation in the failed invocation included the
attributable source anchor in actual provider context and used its synthetic
value in a committed final. This measures existing lexical activation only;
it does not establish trained semantic retrieval.

## Qualified isolated baseline

Run `20260905T040859Z-2ed34a05b4c94f039cc487676764b727` passed all 24 runner
self-checks and both real Keith baseline tests with zero skipped cases. The
recorded setup proved the first-meeting manifest and durable introduction event.
The subsequent ordinary turn committed exactly `4`. The memory probe observed
ingestion before its fresh-session query, included the source entry in actual
provider context, and used the synthetic stored value in its committed answer.
Built and launched binary identities remained stable. This is live baseline
evidence through the Web command API and real provider; it is not browser,
trained-semantic, statistical-regression, or packaged-release qualification.

## Current authoritative record families

| Family | Owner and canonical storage | Projection and write/recovery seam |
| --- | --- | --- |
| Consolidated memory | `memory::MemoryService`, `MemoryLedger` and `MemoryRecord`; `.keith/memory-ledger.json` | `MemoryService::commit_ledger` materializes managed workspace text and atomically persists the ledger. Callers then synchronize records into the evidence vault; service open repeats synchronization. |
| Evidence anchors and direct memory writes | `memory::MemoryObservatory`, `EvidenceRecord`, `VaultEvent`; `.keith/memory-vault.jsonl` | `MemoryObservatory::apply` appends canonical mutations before rebuilding/persisting `.keith/memory-atlas.json`. The atlas and hot cache are projections. Direct `memory_write`, `memory_correct` and `memory_forget` operate through this vault. |
| Explicit relationship onboarding/name | `memory::RelationshipService`; `.keith/relationship-events.jsonl` | `RelationshipMutation` records introduction/name confirmation/forgetting. `sync_evidence` projects those transitions into the evidence vault. This service is not a general graph of causal relationships. |
| Session history and finality | `session-store::SessionWriter`, `SessionEntry`, `SessionManifest`; session `history.jsonl` and `manifest.json` | Append-only entries retain tool calls/results, final candidates, committed finals, obligations, delivery outbox and terminal snapshots. Context reconstruction and indexes derive from this history. |
| Queued task lifecycle | `action-store::PersistentActionInbox`, `SessionAction`, `ActionRecord`; `PendingActions` via `ActionRepository` | Admitted/running/waiting/completed action state uses repository revisions. One queued action is not one tool execution attempt. |
| Existing operation reconciliation | `daemon-core::OperationJournal`, `DurableOperation`; `ActiveOperations` in the state database | Stable operation identity, retry policy and durable boundaries survive restart. Ordinary tool dispatch wiring to this journal was not found in the inspected path. |
| Scheduled work | `scheduler::Scheduler`, `ScheduledJob`, `JobAttempt`; `ScheduledJobs` and `JobAttempts` | Transactional claims retain action identity, lease, schedule and retry/backoff. Enqueue passes through the existing action inbox. |
| Experience telemetry | `evolution::ExperienceService`, `ExperienceRecord`; `ToolExperienceRepository` | Current records support provider/tool/skill outcome and latency history. Conditional causal procedures require additional source and applicability metadata. |

Source locations:

- [Memory ledger and service](../../crates/memory/src/lib.rs): `MemoryRecord` at
  84, `MemoryLedger` at 113, `MemoryService` at 175, `open` at 203,
  `commit_ledger` at 452, `persist_ledger` at 832.
- [Evidence vault](../../crates/memory/src/observatory.rs): `EvidenceRecord` at
  114, `VaultMutation` at 357, `VaultEvent` at 385, `MemoryObservatory::open` at
  444, `apply` at 481, `sync_memory_records` at 548.
- [Direct memory operations](../../crates/memory/src/unified.rs):
  `memory_write`, `memory_correct`, `memory_forget`, `memory_context`.
- [Relationship log](../../crates/memory/src/relationship.rs):
  `RelationshipMutation` at 57, `RelationshipService` at 160.
- [Session records](../../crates/session-store/src/lib.rs):
  `SessionEntryPayload` at 271, `SessionEntry` at 427, `SessionWriter::append`
  at 1665, `append_finalized_turn` at 1708.
- [Action records](../../crates/action-store/src/lib.rs): `SessionAction` at
  208, `ActionRecord` at 265, `PersistentActionInbox` at 346.
- [Recovery journal](../../crates/daemon-core/src/recovery.rs):
  `DurableOperation` at 106, `OperationJournal` at 147.
- [Scheduler](../../crates/scheduler/src/lib.rs): `ScheduledJob` at 61,
  `JobAttempt` at 105; [experience](../../crates/evolution/src/lib.rs):
  `ExperienceRecord` at 73.

The consolidation ledger cannot reconstruct every evidence anchor: direct
memory writes bypass it. The vault is not a disposable index. New indexes must
remain subordinate to the authoritative record family and its correction and
deletion history.

## Actual automatic activation path

1. `local-runtime::LocalRuntime::run_turn` assembles the normal agent turn.
   Profile modules open `MemoryService`; the `ProfileModules::open` parameter
   named `_retrieval` is unused for constructing memory at the inspected seam
   (`crates/local-runtime/src/lib.rs:1307`).
2. Context construction enqueues committed session evidence, creates an
   `ActivationRequest`, and calls `memory::select_activation` directly
   (`crates/local-runtime/src/lib.rs:5947`).
3. `select_activation` reads the archive revision and calls
   `MemoryObservatory::search` (`crates/memory/src/activation.rs:88`).
4. `MemoryObservatory::search` computes lexical and character-trigram scores,
   combined as `0.6 * lexical + 0.4 * trigram`
   (`crates/memory/src/observatory.rs:742`). This inspected selection path does
   not call a trained embedding encoder.
5. `validate_activation` checks the archive revision, source identities/digests,
   current validity and sensitivity before reuse
   (`crates/memory/src/activation.rs:220`).
6. Local-runtime serializes a nonempty valid `MemoryActivationManifest` into
   provider system context using `ContextProvenance::RetrievedMemory` and
   `PersistPolicy::Never` (`crates/local-runtime/src/lib.rs:5964`). The source ID
   is `memory_activation:<manifest_id>`.

The separate [retrieval crate](../../crates/retrieval/src/lib.rs) has
`Embedder` at 127, `RetrievalService::search` at 354, `vector_scores` at 468 and
`LocalHashEmbedder` at 572. `LocalHashEmbedder::embed` hashes lexical features;
the existence of this vector interface does not establish trained semantic
recall in automatic memory activation.

`MemoryActivationManifest` already resides in session-store at 952. It includes
query identity, archive revision, selected evidence, coverage and token price.
It does not currently identify encoder/index generation, required binding
coverage, or separately record downstream actual inclusion/use. Extend this
contract rather than invent a second selection manifest.

## Tool action, effect and restart path

- `AgentLoop::commit_assistant_activity`
  (`crates/agent-loop/src/lib.rs:755`) durably records the assistant activity and
  each `SessionEntryPayload::ToolCall` before subsequent execution.
- `tool-core::ToolManager::invoke` at 413 owns tool readiness, execution rules,
  confirmation and bounded attempts. `ToolEventKind::Started { attempt }` is an
  event emitted by the manager; the inspected type alone does not make each
  attempt durable.
- Effect vocabulary already exists as `agent-types::ToolEffectState` at 430:
  `NotStarted`, `NotCommitted`, `Committed`, `Unknown`.
- The retry condition at `crates/tool-core/src/lib.rs:555` requires a
  state-writing tool to report `NotCommitted` before that invocation retries.
  `normalize_effect_state` at 855 disables automatic retry for unknown effects.
- `AgentLoop::commit_tool` at 827 persists a `ToolResult` with output/error and
  optional `ToolFailure` after execution. Invocation-local attempt counts do
  not establish persistent convergence history across turns or compaction.
- `local-runtime::repair_unknown_tool_outcomes` at 7538 appends an explicit
  `TOOL_OUTCOME_UNKNOWN` result when restart finds a tool call without a durable
  result. It instructs state inspection before retry. There is no inspected
  prediction record or separate durable dispatch marker to distinguish an
  admitted but undispatched proposal from the ambiguous dispatch window.

### Existing operation journal and integration gap

`OperationJournal::begin` at 168 is intended to persist an operation before
execution and return an existing matching stable identity. `commit_boundary` at
210 advances `Started`, `EffectObserved`, and `FinalCommitted` boundaries.
`reconcile` at 235 invokes `ExternalStateProbe` when the retry policy requires
authoritative readback. `recover_worker` at 441 calls journal reconciliation at
469.

In the searched production tool path, `OperationJournal` is not called by the
ordinary `agent-loop` dispatch/commit methods. Searches found journal usage in
the recovery module and its tests; this is an observed wiring gap, not proof
that every possible runtime integration is disconnected.

`DurableOperation` lacks explicit profile/task/turn/tool-call linkage,
prediction references, world/capability versions, target/precondition revisions,
and `ToolEffectState`. These fields need an agreed existing owner before wiring
causal execution. Lower layers must not depend on daemon-core simply to reuse a
record; shared contracts should move to their appropriate existing lower layer.

There is also a collection compatibility concern:

- `OperationJournal::operations` at 259 attempts to decode every record in
  `Collection::ActiveOperations` as `DurableOperation`.
- Local-runtime uses that same collection for other record shapes, including
  `set_background_control` at 3726 and confirmation state near 3770.
- Reusing the journal with a database containing these records therefore needs
  explicit record discrimination. A runtime regression must exercise that mixed
  collection; an empty isolated journal would not prove compatibility.

## Finalization and delivery remain separate

`AgentLoop::commit_final_candidate` (`crates/agent-loop/src/lib.rs:797`)
persists a candidate via `SessionWriter::append_final_candidate`; a candidate
is not the committed user answer.

The ordinary local-runtime turn calls `SessionWriter::append_finalized_turn`
at `crates/local-runtime/src/lib.rs:2293`. That session-store method persists
the committed final, outbox/obligation/terminal records and authoritative
snapshot under existing validation. `AuthoritativeTurnSnapshot`
(`crates/session-store/src/lib.rs:250`) distinguishes execution success,
final creation, artifacts persisted, delivery enqueued and delivery acknowledged.
Local-runtime emits `AssistantFinalCommitted` from the returned committed final
near 2417, after finalization.

New prediction assessment must not reinterpret a final candidate as delivered,
merge delivery acknowledgement with task success, or create another finalizer.
Source inspection does not qualify transport delivery or behavior during every
finalization failure window.

## Scheduled and delayed work path

Local-runtime calls `Scheduler::tick_sessions` for a root-scoped worker or
`Scheduler::tick` otherwise (`crates/local-runtime/src/lib.rs:5288`). Both
converge on `tick_filtered` (`crates/scheduler/src/lib.rs:343`).

`claim_job` at 575 transactionally stores schedule advancement and attempt
records. `retry_due` at 635 restores expired claims and due backoff. The attempt
retains its `action_id`; `enqueue_attempt` at 673 creates a `SessionAction` and
uses `ScheduledActionSink`, implemented by `PersistentActionInbox` at 176.
Repeated enqueue accepts `AlreadyPresent` instead of inventing another action
identity.

`recover_daemon_startup` (`crates/daemon-core/src/recovery.rs:481`) migrates
state, expires scheduler/delivery claims, rebuilds the catalog, and restores
schedules, waits, deliveries and channel offsets. Expired delivery claims retain
possible-duplicate uncertainty when the platform lacks idempotency.

These are the existing homes for delayed assessments and projection execution.
Canonical memory JSONL writes and scheduler database writes are different
durability boundaries: source revision replay or a documented reconciliation
window is needed; a shared transaction cannot be assumed.

## Contract placement and historical compatibility

`crates/framing/src/lib.rs` contains `LengthDelimitedCodec` for bounded byte
transport. It does not contain world frames or model context contracts. A new
`WorldFrame` must not be placed there because of the crate name.

`provider-core` currently owns `CancellationToken`, `ProviderCredential`,
`ProviderError` and `ModelProvider`, but no trained embedding contract.
`configuration::AgentProfile`/`ModelRoute` own current chat selection and its
credential reference (`crates/configuration/src/lib.rs:310` and 333).
Independent embedding selection is an addition, not a property established by
the existing `RetrievalConfig.vector` boolean.

The executable dependency rules in `apps/xtask/src/main.rs:614` forbid domain
reachability into provider-adapters and forbid provider-adapters from reaching
session-store. Consequently a provider adapter must implement a provider-core
embedding contract; it cannot import a retrieval adapter that transitively
reaches memory/session-store. Memory can own the semantic candidate port while
outer composition supplies its retrieval implementation.

Historical serialization needs special care:

- `SessionEntry::verify` (`crates/session-store/src/lib.rs:470`) reserializes
  parsed payloads to verify their checksum.
- `load_vault` (`crates/memory/src/observatory.rs:1174`) verifies schema,
  sequence, previous digest and `event_digest`; `event_digest` at 1351 hashes a
  canonical serialization including each parsed mutation and evidence record.
- Adding a field with a deserialization default can still alter historical
  canonical bytes if serialization emits that default. Absent optional fields
  must remain omitted when preserving old checksum/digest representations, or
  be handled by an explicit versioned migration/legacy reader.
- Unsupported versions must fail explicitly. Missing legacy causal metadata
  must remain unknown rather than acquire invented observation provenance.

Required compatibility fixtures should come from real serialized baseline
types and reopen through the real stores. They should cover old chains,
correction/deletion, mixed operation records, profile mismatch, committed-vault
projection failure and restart replay.

## Gaps to qualify after this source baseline

1. Automatic memory activation lacks a trained semantic candidate path in the
   inspected wiring; provider switching and honest degradation need actual-turn
   probes.
2. Required object bindings, semantic generation identity, source-root
   independence and selected-versus-included evidence need additive contracts.
3. Canonical commit/projection failure windows require idempotent repair;
   in-memory pending ingestion is not a durable projection queue.
4. Ordinary tool attempts need prediction and dispatch lineage linked to the
   existing recovery owner, with mixed-record compatibility established.
5. Attempt counts currently do not prove durable strategy convergence through
   session restart or compaction.
6. Inference, conditional procedure and latent-pattern contracts must preserve
   source authority without replacing existing finalization or approval owners.
7. Runtime, fault-process, provider, packaged, browser and cross-client evidence
   remain separate qualification work. None is passed by this source document.
