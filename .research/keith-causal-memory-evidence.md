# Keith causal memory: evidence and limits

Date: 2026-09-05. Research support for the [design](keith-world-and-reconstructive-memory.md) and [implementation plan](keith-causal-memory-implementation-plan.md).

This ledger records what informed the plan. Historical handoffs are reports from their authors; their “fixed,” “green,” and “live-verified” labels are not fresh verification. Source inspection establishes the inspected implementation, not a running deployment. Proposed requirements are conclusions to test.

## Historical evidence

| ID | Record and observation | Consequence for the plan |
|---|---|---|
| H01 | The May 25 session export contains a user-supplied Cortex failure: “Near/NearURI requires StartEmbedder (no embedder running).” The contemporary diagnosis identifies an unset deployment flag. Later messages report the embedding error cleared, but the full task still reached a separate sub-dispatch failure. [Dataset session](/root/dataset/sessions/defiant-push.json), embedded original messages 16, 22, 24, 29, 109. | Capability implementation and deployment activation need separate proof. HTTP success or a completed pipeline does not prove task success. This is an earlier Matrix execution-path incident, not proof of the later Neo migration cutoff. |
| H02 | July 3 handoff reports the Z.ai switch and Railway migration code; July 5 history records a subsequent xAI migration, with a July 13 update recording MiMo chat lanes. [July 3](/root/compaction/memory/handoff-zai-railway-20260703.md:22), [provider history](/root/compaction/memory/xai-migration-20260705.md:13). | Preserve embedding configuration, dimensions, credentials references, indexing, and query wiring independently of chat-provider configuration. The records do not establish the exact deployment when semantic memory was lost. |
| H03 | July 22 Moltbook incident: the correct endpoint was stored three hours before the failure; fresh scheduled conversations depended on actor-global Cortex retrieval. Indexing latency and bounded ranking could omit the fact. [Record](/root/compaction/memory/neo-moltbook-recall-poisoning-20260722.md:13). | Required task bindings need direct lookup and freshness checks; optional semantic ranking cannot be their only route into context. |
| H04 | The same incident records a guessed domain becoming false durable “service defunct” knowledge. Contradictions were neighbor-gated and could remain unlinked. The reproduction described working semantic retrieval under limited conditions; loaded-corpus ranking remained open. [Record](/root/compaction/memory/neo-moltbook-recall-poisoning-20260722.md:18). | Distinguish source observation from its interpretation. Resolve corrections and conflicts by entity/property identity when available, not only vector proximity. Retain unknowns. |
| H05 | July 22 close-loop incident: rejected answers remained in the working window and durable transcript as though delivered; an overflow guard repeatedly rejected closing; unsupported MiMo call grammar fed the loop. [Record](/root/compaction/memory/neo-loopty-loop-fix-20260722.md:13). | Preserve candidate, committed-final, and delivery states. One finalization owner consumes checks. Provider conformance must be exercised through actual multi-turn behavior. |
| H06 | July 12 money-path incident: a pinned rule required a removed tool. The agent searched for that nonexistent capability and improvised a materially different operation. [Record](/root/compaction/memory/money-path-honesty-20260712.md:14). | Bind procedural knowledge to capability versions. The currently advertised and authorized contract determines what exists; remembered instructions cannot recreate a removed capability. |
| H07 | The July 12 epistemic design explicitly required grounded premises, prediction-carrying dispatch, mismatch-driven revision, and evidence-based progress. [Design handoff](/root/compaction/memory/epistemic-core-spec-20260712.md:12). | Preserve the teaching intent, but prove that the ordinary action path actually uses it. Failed assessment must not silently count as confirmation. |
| H08 | July 17 O1 audit found real package code and tests, but most policy machinery was initially disconnected from the live runtime despite completed task labels. Initial integration also over-restricted the visible tool surface. [Audit](/root/compaction/memories/rollout_summaries/2026-07-17T17-58-59-jIAV-architect_o1_runtime_wiring_and_invisible_rigor.md:25). | Package tests are necessary but insufficient. Every phase needs a real turn-path demonstration and an ordinary-task regression check. |
| H09 | Compacted known gaps include unsupervised extraction into pinned blocks, deduplication falling back to “new,” and learned behavior silently dropping out of top-eight selection. [Known gaps](/root/compaction/recall.md:395). | Exposure is not usefulness; extraction is not confirmation; corrections and obligations need priority independent of optional relevance scoring. |
| H10 | July 16 readiness report records structured application errors being treated as success when the shell exited zero. [Report](/root/compaction/memories/rollout_summaries/2026-07-16T21-01-33-XAHw-neo_beta_launch_readiness_report.md:23). | Keep transport/process success, effect state, and user-goal satisfaction separate. |
| H11 | July 17 product direction required reliability machinery to remain internal while Neo felt intelligent and flexible. [User-direction record](/root/compaction/memories/rollout_summaries/2026-07-17T17-58-59-jIAV-architect_o1_runtime_wiring_and_invisible_rigor.md:57). | Measure unnecessary interruption, ordinary-task success, and useful initiative alongside invariant enforcement. |

The general recall file is a reorganized compaction, not a reliable event chronology by itself. For example, it places a Moltbook-class fix under “Late June,” while the dedicated incident note dates its account July 22. Prefer the specific record and disclose unresolved chronology.

The user's account of embedding loss sets the investigation priority. H01–H04 support making semantic grounding foundational. They do not support claiming that all Neo failures had a proven single cause, that embeddings were absent throughout July, or that the later fixes were production-qualified in this review.

## Current Keith source observations

Scope: the user-selected checkout `/root/agent-keith`, inspected on 2026-09-05. An older memory named `/root/project-paperclip/agent-keith`; that path is absent on this host. No other checkout was substituted.

| ID | Observed source | What it establishes / what it does not |
|---|---|---|
| K01 | [Dependency boundaries](../docs/architecture/dependency-boundaries.md), [crate ownership](../docs/crate-guide.md), [security boundaries](../SECURITY.md). | Existing contracts, domains, adapters, and orchestration are the placement rules. This proposal must not create another daemon, security authority, or memory owner. |
| K02 | [EvidenceAuthority and EvidenceRecord](../crates/memory/src/observatory.rs), [unified memory](../crates/memory/src/unified.rs). | Provenance classes, source digests, correction chains, sensitivity, and profile scope exist. Exact quote attribution is useful, but a quoted model assertion is still a model assertion. |
| K03 | [LocalRuntime activation](../crates/local-runtime/src/lib.rs), `ActivationRequest` around line 5947; [select_activation](../crates/memory/src/activation.rs), around line 88; [MemoryObservatory::search](../crates/memory/src/observatory.rs), around line 742. | The inspected automatic activation path invokes observatory search, which scores lexical and trigram overlap. It does not demonstrate learned semantic embedding retrieval. |
| K04 | [Retrieval implementation](../crates/retrieval/src/lib.rs), `Embedder` around line 127, `vector_scores` around line 469, `LocalHashEmbedder` around line 572, `semantic_features` around line 1186. | Optional vectors and an embedding interface exist. The local implementation hashes word prefixes and trigrams; it is not a trained semantic encoder. This differs from Neo's whole-text pseudovector fallback and should not be inaccurately described as identical. |
| K05 | [Memory documentation](../docs/features/memory-and-retrieval.md), [MemoryService ledger](../crates/memory/src/lib.rs). | Memory already has an authoritative ledger, materialized views, and default lexical/trigram lookup. Vector lookup is documented as optional/off by default. The new plan intentionally proposes a stronger semantic capability requirement; that requirement is not current behavior. |
| K06 | [ToolManager](../crates/tool-core/src/lib.rs), retry branch around line 553; [ToolEffectState](../crates/agent-types/src/lib.rs), around line 430; [delivery recovery](../crates/delivery/src/lib.rs), around line 398. | Typed effects and restrictions on retrying unknown writes already exist. Extend and trace these mechanisms rather than replacing them. Their existence does not prove every shell/browser side effect is reconciliable. |
| K07 | [Agent loop](../crates/agent-loop/src/lib.rs), [session](../crates/session/src/lib.rs), [local runtime](../crates/local-runtime/src/lib.rs). | Candidate-final and committed-session concepts already exist. The plan must trace finalization, ingestion, client projection, and channel acknowledgement before changing their semantics. |
| K08 | [Everyday evolution](../crates/evolution/src/lib.rs), [task recipes](../crates/task-recipe/src/lib.rs), [meta-harness](../crates/meta-harness/src/lib.rs), [self-evolution guide](../docs/features/self-evolution.md). | There are existing learning, teaching, and governed code-change homes. General causal learning and its promotion criteria remain proposed work. |

The source map is deliberately narrow. It is not a release audit or an assertion that every caller has been traced. Phase 0 closes those gaps before implementation claims.

## External research and provider references

- **R01 — [Compiled World Runtime](https://compiled-world.sites.paxeer.app/), user-authored proposal.** Separates durable state from a versioned action ontology and specifies reconciliation across upgrades. Keith's capability-epoch adaptation is a proposal in our plan, not a demonstrated implementation of the entire paper.
- **R02 — [Latent Relatedness](https://latent-relatedness.sites.paxeer.app/), user-authored paper.** Separates inference capacity from scaffolding that maintains a revisable user model. Using inferred priorities to choose anchors is our testable extension; the paper does not establish its performance in Keith.
- **R03 — [LongMemEval](https://arxiv.org/abs/2410.10813).** Evaluates extraction, reasoning across sessions, temporal reasoning, knowledge updates, and abstention; it distinguishes indexing, retrieval, and reading. Use those categories to complement Keith-specific execution cases. Its reported results are not Keith results.
- **R04 — [Reflexion](https://arxiv.org/abs/2303.11366).** Studies feedback and episodic reflection without weight updates. It motivates a comparison arm, not a guarantee that reflective text is true or transfers safely.
- **R05 — [Voyage embedding documentation](https://docs.voyageai.com/docs/embeddings).** Documents query/document input modes, selectable dimensions, and model choices including `voyage-4-large` and open-weight `voyage-4-nano`. This supports an initial benchmark candidate and an offline candidate; provider descriptions do not decide the winner. Recheck exact model availability when implementing.

The two user papers were accessible through direct HTTPS when the browser tool could not open them. Primary research abstracts and provider documentation were checked on 2026-09-05. No provider was purchased, configured, or exercised with profile data.

## Unresolved evidence

1. Exact production configuration and index state at the Fireworks/hosting cutover.
2. Which current Keith surfaces share the inspected activation path and which use separate search adapters.
3. Real semantic-recall quality, latency, and freshness on the user's data distribution.
4. Whether reconstruction adds decision value beyond good anchor retrieval and conditional procedures.
5. Whether latent-pattern retention beats explicit corrections, recency, and frequency on future tasks at equal storage.
6. Whether broader procedure transfer improves behavior without inappropriate generalization.

Do not fill these gaps with a stronger narrative. Each maps to a proposed probe or an explicitly bounded claim in the plan.
