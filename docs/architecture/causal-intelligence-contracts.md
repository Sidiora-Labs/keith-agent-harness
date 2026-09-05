# Causal intelligence contracts and ownership

Task 1.3 adds bounded contracts and proves compatibility with serialized records
captured from the existing stores. It does not wire trained semantic retrieval,
prediction, assessment, projection jobs, or world admission into the runtime.
The source baseline is commit `3c70d2c8bf4ea57b8f261b67bc11d67ee801102c` plus the
working-tree task 1.1 repairs; [baseline evidence](causal-intelligence-baseline.md)
records that distinction. Symbols below are lookup anchors rather than promises
about unchanged line numbers.

## Existing authority remains singular

| Record family | Canonical owner and persistence | Write and reconstruction seam |
| --- | --- | --- |
| Consolidated memory | `MemoryService`, `MemoryLedger`, `MemoryRecord` in [memory/lib.rs](../../crates/memory/src/lib.rs); `.keith/memory-ledger.json` | `commit_ledger` materializes managed workspace text and atomically persists the ledger. `sync_memory_records` projects these records into the vault; `MemoryService::open` repeats synchronization. |
| Evidence anchors, direct memory additions/corrections/deletions | `MemoryObservatory`, `EvidenceRecord`, `VaultEvent` in [observatory.rs](../../crates/memory/src/observatory.rs); `.keith/memory-vault.jsonl` | `apply` prepares and appends canonical mutations, applies their projection and rebuilds/persists the atlas. `memory_create`, `memory_correct`, `memory_forget` in [unified.rs](../../crates/memory/src/unified.rs) use this owner. The consolidation ledger cannot reconstruct all direct writes. |
| Explicit onboarding and preferred name | `RelationshipService`, `RelationshipMutation` in [relationship.rs](../../crates/memory/src/relationship.rs); `.keith/relationship-events.jsonl` | Introduction/name transitions are canonical here; `sync_evidence` projects them into the evidence vault. This family must not become a second general-purpose causal graph. |
| Session ingress, tool calls/results, finality and delivery intent | `SessionWriter`, `SessionEntryPayload`, `SessionEntry`, `SessionManifest` in [session-store](../../crates/session-store/src/lib.rs); session history/manifest | `append`, `accept_turn`, `append_final_candidate`, `append_finalized_turn` preserve the existing history and terminal rules. Session indexes and context are derived. |
| Whole queued action | `SessionAction`, `ActionRecord`, `PersistentActionInbox` in [action-store](../../crates/action-store/src/lib.rs); `PendingActions` | Repository revisions and inbox transitions own admitted/running/waiting/terminal action lifecycle. An action is not a tool attempt. |
| External operation recovery | `DurableOperation`, `OperationJournal` in [daemon-core/recovery.rs](../../crates/daemon-core/src/recovery.rs); `ActiveOperations` | `begin`, `commit_boundary`, `reconcile` and `recover_worker` own operation identity and reconciliation. Ordinary tool dispatch is not yet connected. |
| Durable jobs and waiting | `ScheduledJob`, `JobAttempt`, `Scheduler` in [scheduler](../../crates/scheduler/src/lib.rs), existing waiting machinery | Existing claim/lease/retry/backoff and action-enqueue ownership is retained. Projection and assessment payloads must extend this path, not create another scheduler. |
| Experience and executable adoption | `ExperienceRecord`, `ExperienceService`, `ToolExperienceRepository` in [evolution](../../crates/evolution/src/lib.rs); [task-recipe](../../crates/task-recipe/src/lib.rs), skills | Experience observations and reviewed procedure adoption remain separate. Code changes still pass through meta-harness/self-evolution governance. |

`.keith/memory-atlas.json`, hot caches and future vector indexes are rebuildable
projections. The evidence vault and relationship log are not disposable indexes.
No new storage collection, operation controller or lifecycle owner is introduced
by task 1.3.

## Every logical record in `design.records`

“Add later” below identifies missing fields and their intended existing owner.
It does not describe a field already serialized by this change. The minimum
shared primitives and memory metadata implemented now are listed afterward.

| Logical record and fields | Existing representation | Minimum addition and owner |
| --- | --- | --- |
| **EvidenceAnchor:** ID, source entries/digests, profile, sensitivity, authority, observed time, validity, correction lineage | `EvidenceRecord.id`, `source_session`, `source_entries`, `source_digests`, `source_identity`, `profile_id`, `sensitivity`, `authority`, `occurred_at`, `validity`, `supersedes`, `superseded_by`, `dispute_reason`, `deleted_at`; source kind, retention and content digest already exist | Reuse `EvidenceRecord` rather than a second anchor type. **Added now:** optional `causal: EvidenceCausalMetadata` containing an effective interval and source roots. Missing metadata remains absent. |
| Anchor recording time, effective interval, independent source-root identity | Enclosing `VaultEvent.occurred_at` is the canonical recording time; it differs from `EvidenceRecord.occurred_at`. Source entries/digests identify immediate sources, which can be summaries or generated views | Recording time stays in the event. `EvidenceEffectiveInterval { from, until }` is half-open; bounds may remain unknown. `EvidenceSourceRoot { source_session, source_entry, source_digest }` records originating entries. Source shape is validated; authenticating those roots against canonical profile history and deduplicating derived views remains later work. No missing roots are guessed. |
| **TaskObjectBinding:** task/scope, canonical entity/property, evidence ID/revision, freshness, resolved/missing/stale/conflicting | Existing `ProfileId`, `WorkspaceId`, `ActionId`, `GoalId`, `EntityId`, `Revision`; `EvidenceRecord` owns validity/corrections. `WorldFrame.object_revisions` now carries bounded entity/property/revision/evidence references across admission | Add later in memory: a versioned binding value and deterministic resolution policy referencing canonical evidence, with explicit freshness and missing/conflicting state. Resolved bindings are projections of evidence. If a binding must be persisted for a particular attempt, persist its frozen reference in that attempt's existing session record. |
| **Hypothesis:** claim, roots, conditions, alternatives, counterevidence, generator/model/representation version, proposed/supported/disputed/retired | `RecallClaim` already holds text, supporting/contradictory evidence and uncertainty; `RecallCapsule`/`MemoryScoutFinding` carry scoped evidence and archive revision. `DerivedInference` already exists as evidence authority | Add later in memory: versioned hypothesis metadata and append-only transitions through the memory evidence owner. Conditions, alternatives, generator identity and inferential lifecycle are missing. Do not overload `EvidenceValidity`: supported inference remains inferential. Recall capsules remain projections, not new authorities. |
| **Relationship:** typed endpoints, temporal/entity/task/evidential/causal/procedural/analogical type, asserted/observed/inferred origin, scope, independent supports, counterevidence, validity/revision | `AtlasNode`, `AtlasEdge`, `AtlasRelation`, facets and evidence references already represent derived navigation. `EvidenceAuthority`, source roots, profile IDs and vault revision provide provenance primitives | Add later in memory: canonical versioned relationship assertions/inferences with endpoint kinds and support/counterevidence roots, via the evidence mutation family; project edges into the atlas. Preserve the separate narrow onboarding/name log. Causal/analogical proposals start inferred; a graph edge cannot upgrade its source authority. |
| **Prediction:** immutable ID/revision, task/attempt/operation linkage, world/capability versions, target/precondition revisions, evidence/hypothesis refs, expected predicates, deadlines, uncertainty | Existing IDs: `EntityId`, `EntryId`, `ToolCallId`, `TurnId`, `ActionId`, `GoalId`; `Revision`, `UtcTimestamp`. Session history owns tool call/result records; `DurableOperation.id`/`stable_identity` owns reconciliation. `WorldVersion`, `CapabilityEpoch`, `WorldFrame` are added now | Add later: bounded prediction/predicate contracts in tool-core where shared at admission, frozen as typed session-store entries before dispatch. Link the existing operation journal; do not copy operation authority into session history. Predictions carry explicit target/evidence revisions, deadline and uncertainty. A superseding unexecuted prediction appends a new ID/revision linked to its predecessor. |
| **Assessment:** prediction/predicate ID, supported/contradicted/pending/unresolvable/not_executed, observation IDs/revisions, evaluator/version, time, unresolved reason | `ToolEffectState` already distinguishes not-started/not-committed/committed/unknown. Session tool results and `AuthoritativeTurnSnapshot` provide observations and separate execution/final/artifact/delivery statuses | Add later: versioned append-only assessment entries in the existing session history, with bounded shared outcome vocabulary at tool-core. Evaluation appends evidence references and later revisions; it never rewrites a prediction or treats a timeout as non-commitment. Index/learning projections consume these entries. |
| **StrategyRecord:** task, target, intended effect, mechanism, preconditions, attempts, roots, failed expectations, remaining budgets, revision lineage | Agent-loop owns convergence; tool-core owns effect-aware retry. Existing `ActionLimits`/loop controls own budgets. Tool calls/results retain source observations; goals retain continuation identity | Add later: durable strategy metadata in the session history, owned by agent-loop transitions, referenced by existing action/goal identity. Preserve existing budget accounting rather than copying mutable remaining budgets into a competing counter. Recorded budget snapshots are descriptive. Revision lineage and structural equivalence survive model renaming and new queued actions under one goal. |
| **ProcedureExperience:** conditional strategy, expected outcomes, adapter/world compatibility, independent supporting episodes, failures/counterexamples, applicability, adoption history | `ExperienceRecord` currently contains profile, task category, subject, outcome, latency and observed time. `ExperienceService`/repository own experience; task-recipe and skills own reviewed executable adoption | Add later in evolution: versioned conditional experience extensions referencing strategy/prediction/assessment/root IDs, world/adapter versions, support/failure episodes and applicability. Reuse the experience repository with record-kind discrimination. Task-recipe/skills record reviewed adoption; generated experience alone never installs an executable procedure. |
| **RetrievalManifest:** profile/task, query representation digest, canonical source revision, encoder/index generation, candidate lanes, selected IDs/revisions, coverage/gaps, actual included IDs, degraded reasons, later use | `session-store::MemoryActivationManifest` already has `profile_id`, `session_id`, `query_identity`, `archive_revision`, `evidence`, `coverage`, `token_price`, `selector_version`, `manifest_id`; selected evidence includes source and content digests, authority and validity | Keep that manifest as the selection record. **Added now:** the memory candidate port carries action/goal scope, query identity, index/space identity, source watermark, ranked reference lanes and degraded reasons. Add later optional absent-preserving manifest metadata for these fields, required binding gaps and actual inclusion at local-runtime assembly. Record downstream use separately; selection/inclusion cannot prove use. No manifest fields were changed in task 1.3. |
| **UserPatternHypothesis:** scoped interpretation, attributable user signals, alternatives/counterexamples, time window, correction/rejection, expiry/review, measured later utility, self-caused markers | Evidence authority/source entries distinguish user assertions from assistant content; `RecallClaim` provides inferential references. Existing explicit relationship name/onboarding state is not an implicit user-pattern store | Add later as a memory-owned hypothesis subtype with signal roots, interval, alternatives/rejections, review/expiry and measured utility observations. Self-caused interactions need explicit markers. Inferred patterns cannot overwrite explicit preferences or transform repeated generated views into independent user evidence. |
| **Projection/assessment jobs:** profile, source/revision or prediction ID, deterministic identity, lease/attempt, retry/backoff, watermark | `ScheduledJob.version/id/profile_id/session_id/action/limits/state/...`; `JobAttempt.job_id/attempt_id/ordinal/claimed_by/claim_expires/state/action_id/retry_count/retry_at/...`. `Scheduler::claim_job`, `retry_due`, `enqueue_attempt` own scheduling | Add later typed job payloads and deterministic keys from profile + source ID/revision + projection generation (or prediction/predicate ID). Reuse lease/retry controls. Generation watermarks belong to projection state; authoritative sources remain in their original store. No embedding-job intent is written by task 1.3. |

The logical task identity must not reset when a goal continuation creates a new
queued action. Use the existing `GoalId` when present and retain the current
`ActionId` as execution linkage; ordinary standalone actions retain their
`ActionId`. `goals::enqueue_continuation` creates new action IDs, so model labels
or action ID alone cannot identify a goal-wide strategy. No new `TaskId` store
is needed.

## Contracts implemented now

- [agent-types/world.rs](../../crates/agent-types/src/world.rs) owns
  `WorldVersion` and `CURRENT_WORLD_VERSION`. The current reader accepts only
  native semantic version 1.0. `CapabilityEpoch` reuses the monotonic-counter
  representation in agent-types with explicit overflow.
- [tool-core/world.rs](../../crates/tool-core/src/world.rs) owns `WorldFrame`,
  `ObjectRevision`, `PendingEffectReference`, and `CausalRuleOrigin`. A frame
  carries world/epoch/profile/workspace/action/optional goal, bounded object
  revisions, commitment IDs and pending operation references. Checked decoding
  rejects duplicate references, invalid properties, unknown world versions and
  a committed effect represented as pending. It does not grant authority or
  verify current revisions. No old attempt is automatically assigned a frame.
- [provider-core/embeddings.rs](../../crates/provider-core/src/embeddings.rs)
  owns the provider-neutral embedding descriptor, request, response and provider
  trait. `EmbeddingSpaceIdentity` includes contract version, provider, model,
  encoder revision, dimensions, distance, normalization and representation
  version. Query/document roles and response validation belong at this boundary.
- [memory/semantic.rs](../../crates/memory/src/semantic.rs) owns
  `SemanticCandidateSource`, `SemanticCandidateQuery`, `SemanticCandidateBatch`,
  `SemanticIndexIdentity`, `SemanticCandidate`, `CandidateEvidenceReference`,
  lane/degradation enums and contract validation. No candidate contains evidence
  text, truth labels, grants, or raw cross-model similarity scores. `Debug` for
  the query exposes identity and size, not its raw search text.
- [memory/causal.rs](../../crates/memory/src/causal.rs) owns optional anchor
  metadata and its checked versioned reader. Canonical `MemoryObservatory::apply`
  also validates metadata before writing. Existing record construction leaves it
  absent; no automatic inference or migration runs.

The candidate query names the desired generation and complete encoder space.
`validate_for` requires matching profile/session/action/goal/query identity,
space and generation, bounded candidate count, valid digests, positive ranks,
no duplicated hit/rank within a lane, and source/reference revisions that do not
come from the future. A lagging source watermark requires `IndexLag`. Unknown
versions are explicit validation errors. These boundary values must be
validated after deserialization; parsing alone does not certify a candidate
batch. Provider availability and trained-encoder readiness remain independent.

`CandidateEvidenceReference.archive_revision` is the canonical vault snapshot
at indexing time, not a newly invented per-record revision. `resolve_candidate`
reads the vault projection under one lock and checks the requested current
snapshot, canonical profile, ID/content digest, active validity and sensitivity.
It rejects deleted, superseded and disputed evidence. An older index reference
can resolve only if the canonical record remains eligible and unchanged at the
requested snapshot. The method returns canonical evidence; the index supplies
no authoritative text. Later required-binding and effective-time resolution
must add their own checks. No caller is connected to this helper yet.

`framing` remains the length-delimited byte transport codec. It receives no
world frame, memory schema or domain policy.

## Dependency direction

The existing `memory -> provider-core` and `memory -> session-store` edges are
reused; task 1.3 adds no Cargo dependencies. Retrieval can implement the
memory-owned candidate port through `retrieval -> memory`. Embedding adapters
implement the provider-owned transport trait through `provider-adapters ->
provider-core`. Orchestration composes these services.

Do not import retrieval into provider-adapters when that would create a
transitive `provider-adapters -> session-store` dependency. Do not import
daemon-core into memory/tool-core to reach `OperationJournal`. Shared operation
values needed below orchestration must move to an existing lower contract
owner, while the durable journal remains singular. `cargo dependency-policy`
is the required executable check; no exception is introduced here.

## Persistence and recovery compatibility

Session and vault digests are recomputed by reserializing parsed types. Merely
adding `serde(default)` is insufficient when serialization then emits a new
default-valued field. `EvidenceRecord.causal` uses both `default` and
`skip_serializing_if = "Option::is_none"`. Optional effective metadata and goal
references also preserve absence. No existing manifest field was added.
`WorldVersion` and causal metadata reject unsupported versions rather than
assuming today's semantics for an older or future value.

The vault previously treated any typed decoding failure on an unterminated
last line as a torn write. A syntactically complete JSON record with unknown
metadata could therefore be discarded. Recovery now uses JSON syntax only to
identify a damaged final tail; a complete unknown-version record reaches the
normal typed reader and fails closed without changing the file. This behavior
is covered with a checksum-correct future-metadata final record lacking a
newline. Existing incomplete-tail recovery remains covered by the package.

Fixtures were captured **before** changing `EvidenceRecord` using the public
real `SessionStore` writer, accepted turn, tool call/result, final candidate and
`append_finalized_turn`; then public `MemoryObservatory::ingest_session_entries`
and `select_activation`. Content is synthetic, and this capture does not claim
a model/tool adapter invocation. Exact original bytes, generator source and
SHA-256 provenance are retained in
[memory fixtures](../../crates/memory/tests/fixtures/causal-v1/provenance.json)
and [session fixtures](../../crates/session-store/tests/fixtures/causal-v1/).

[Session compatibility tests](../../crates/session-store/tests/causal_legacy.rs)
copy the original files into a real store, reopen, verify every checksum and
reconstruct the complete finalized branch without rewriting its bytes.
[Memory compatibility tests](../../crates/memory/tests/causal_contracts.rs)
reopen the original vault, compare exact evidence/activation serialization,
rebuild corrupt/missing atlas projections, append metadata-bearing correction
and deletion events, and prove no old reference is resurrected. They also reject
unsupported metadata and canonical writes with invalid metadata.
[Candidate contract tests](../../crates/memory/tests/semantic_contracts.rs)
exercise scope, identity, lag, malformed references, bounds, versions and query
debug redaction. They are contract tests, not an adapter readiness benchmark.

## Open integration obligations

1. **Canonical commit versus projection intent:** vault `apply` durably appends
   events before atlas persistence. Ledger commits precede vault synchronization;
   service reopen repeats synchronization. There is currently no atomic
   canonical-write plus embedding-job transaction. Later work must record an
   explicit source-revision reconciliation window and replay incomplete jobs.
   Projection failures must not turn a committed canonical write into permission
   to repeat a side effect.
2. **Operation journal discrimination:** `OperationJournal::operations` reads
   every `ActiveOperations` value as `DurableOperation`, while local-runtime
   stores background control and confirmation shapes there. Wiring must add
   record-kind discrimination and a mixed-collection compatibility test before
   relying on recovery. Empty-store journal tests alone cannot prove this path.
3. **Attempt and dispatch durability:** ordinary attempts currently use session
   `ToolCall`/`ToolResult`; tool-core attempt events are not a durable attempt log.
   `repair_unknown_tool_outcomes` conservatively repairs calls without results.
   Later prediction integration must preserve `ToolEffectState`, operation
   identity and dispatch ambiguity rather than adding an independent retry
   controller. The active journal lacks explicit profile/action/goal/turn/call,
   prediction and world linkage today.
4. **Finality:** `append_final_candidate` is not user delivery.
   `append_finalized_turn` and `AuthoritativeTurnSnapshot` retain execution,
   artifact, final and delivery distinctions. Assessment must reference actual
   durable observations without changing these terminal rules.
5. **Activation:** local-runtime currently calls `select_activation`, which
   uses lexical/trigram `MemoryObservatory::search`. Existing `LocalHashEmbedder`
   does not establish trained semantic recall. The new port is deliberately
   unwired until provider configuration, indexing and actual-context proof are
   implemented in their later tasks.

Focused package results and dependency-policy output belong to task 1.3's
qualification artifact. This document records ownership and compatibility, not
completion of any later wave or production qualification.
