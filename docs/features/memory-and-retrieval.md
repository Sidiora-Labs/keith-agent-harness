# Memory and retrieval

Keith's memory system turns selected, attributable evidence into profile-scoped
records that can be corrected, superseded, searched, and forgotten. It is not a
verbatim transcript pasted into every prompt. At each model step, the runtime
builds a bounded context bundle from relevant records and freezes the selected
revision for that step.

Memory is supporting evidence, not authority. A remembered instruction,
retrieved Web page, or prompt-like sentence in history cannot grant a tool,
change a security rule, or speak as the current user.

## What you can do

- Search memory from the Web Memory panel or the terminal with `/memory QUERY`.
- Let the runtime propose durable preferences, personal facts, project context,
  routines, relationships, commitments, procedures, and preferred names from
  committed conversation evidence.
- Have Keith create, search, inspect, correct, forget, and assemble memory
  context through bounded agent tools.
- Correct an earlier memory without silently rewriting its provenance.
- Let the bounded runtime memory bridge inspect active, disputed, superseded,
  or deleted evidence and rebuild its search atlas from the authoritative vault.
- Let the runtime run deeper read-only recall over large history when the normal
  hot context is insufficient.
- Keep memories, relationship facts, search results, and activation state
  isolated by profile.

The terminal query currently caps its interactive result list at 20. Agent-side
context construction has a separate explicit token budget and sensitivity
ceiling.

## The memory layers

Keith separates several jobs that are often collapsed into one database:

1. **Committed evidence.** Session entries and other accepted sources establish
   what was actually observed.
2. **Memory records.** A ledger stores typed proposed, active, superseded, and
   deleted records, including the source session, source entry, boundary,
   sensitivity, retention, and correction chain.
3. **Human-readable workspace memory.** The personal workspace materializes
   `MEMORY.md` and dated memory files for inspection and portability.
4. **Retrieval indexes.** Lexical and trigram indexes are enabled by default.
   Vector retrieval is optional and disabled by the secure defaults.
5. **Activation.** Relevant records are deduplicated, corrections take
   precedence, and a revision-and-digest-checked bundle is frozen for one
   provider step.
6. **Deep recall.** Bounded read-only scouts can search wider history and return
   exact citations when the ordinary activation set is not enough.
7. **Observatory.** A rebuildable atlas offers catalog, search, timeline,
   evidence expansion, and comparison views over the authoritative evidence
   vault.

```text
committed session evidence
          |
 proposal / correction / forgetting
          v
 profile memory ledger ----> observatory evidence vault
          |                         |
 retrieval indexes           rebuildable atlas
          |                         |
          +---- bounded activation -+
                        |
            frozen context bundle
                        |
                  provider step
```

## Agent memory tools

The local runtime registers these memory tools when the profile permits them:

- `memory_create`
- `memory_search`
- `memory_get`
- `memory_correct`
- `memory_forget`
- `memory_context`

Create, correction, and forgetting operations require a committed source entry
ID and an exact supporting quote. This makes a memory auditable and prevents a
model from inventing provenance for its own assertion.

`memory_context` requires a non-empty query, an explicit sensitivity bound, and
a token budget from 128 through 16,000 tokens. Its result can include
corrections, contradictions, gaps, and coverage information. Deeper recall is
optional rather than an automatic scan of all history.

## Storage and configuration

The profile's authoritative ledger is stored under its Keith data and workspace
layout; the current implementation uses `.keith/memory-ledger.json` and
materializes readable memory files in the personal workspace. Do not edit the
ledger concurrently with the daemon. Use the product's memory operations for
changes so revisions and provenance remain consistent.

Secure retrieval defaults enable lexical and trigram lookup and disable vector
lookup. Installation and profile configuration may opt into additional indexes
where the implementation and local data policy support them.

Automatic retention is bounded. The default policy refuses automatic retention
of secret candidates, allows automatic sensitivity only through `Personal`, and
applies separate current, daily, and durable byte limits. These limits are
safety ceilings, not targets to fill.

## Authority and privacy

- All memory services require a profile scope. Cross-profile search, activation,
  observatory access, and recall must fail closed.
- A memory carries provenance and sensitivity. Retrieval does not erase either.
- Prompt-like text found in old messages remains quoted data; it is not promoted
  to a system or user instruction.
- A relationship fact is learned only from genuine user ingress. Account
  metadata is not used to infer a person's name or relationship.
- Corrections supersede earlier records while retaining the chain needed to
  explain the current answer. Forgetting removes the record from active use and
  propagates to rebuildable views.
- Read-only recall workers receive a bounded scope manifest and cannot write to
  the workspace, message the user, or widen their search authority.
- Secret-looking candidates are not automatically retained. Credentials belong
  in the credential store, never in memory files.

## Failure and recovery

- Automatic ingestion is fail-open for the conversational turn: if memory
  storage fails, the final response and delivery can still complete. Failed
  pending work is restored for later handling rather than presented as stored.
- Retrieval index corruption is quarantined and can fall back to available safe
  lookup modes. The index is rebuildable; the evidence ledger is authoritative.
- Activation validates record revisions and content digests. Stale or changed
  records are omitted rather than injected under an old citation.
- Recall is bounded by depth, concurrency, time, result count, and token count.
  Timeout, stale scope, or cross-profile access fails without leaking partial
  unauthorized results.
- Corrupt relationship state fails open for the chat path and does not become a
  reason to block the user's turn.
- Observatory projections can be rebuilt from the vault after loss or schema
  change.

## Current limitations and status

- Memory improves continuity but does not guarantee that every useful detail is
  retained or selected for every request. Retrieval remains relevance- and
  budget-dependent.
- The human-readable files are not a replacement for provenance in the ledger,
  and the complete `MEMORY.md` is intentionally not copied into each prompt.
- Vector retrieval is off by default. Enabling it adds operational and privacy
  considerations and should not be treated as required for normal use.
- Deep recall is bounded and can return an honest gap instead of fabricating an
  answer from incomplete evidence.
- The source tree contains substantial unified-memory, activation, recall, and
  observatory implementation and focused tests. The corresponding unified
  memory authority and hot-context integration task is still marked in progress
  in the feature spec, so this area should not yet be described as fully
  release-qualified end to end.

## Validate changes

Memory changes should cover:

- create, search, inspect, correct, supersede, forget, and restart behavior;
- exact source evidence, quote validation, revision freezing, and stale-record
  rejection;
- sensitivity ceilings, secret rejection, retention bounds, and profile
  isolation;
- relevant activation, deduplication, correction precedence, contradictions,
  and an honest empty result;
- index corruption and rebuild from authoritative evidence;
- bounded recall cancellation, timeout, crash replay, stale scope, and exact
  citations;
- prompt-injection-shaped historical text remaining untrusted data;
- memory storage failure not suppressing a committed user turn or final answer.

When changing a client, also exercise the real native memory query rather than
only testing presentation helpers.

## Key source locations

- `crates/memory/src/lib.rs` — memory service, records, policy, persistence, and
  relationship memory
- `crates/memory/src/unified.rs` — unified memory operations and evidence rules
- `crates/memory/src/activation.rs` — bounded, revision-frozen context selection
- `crates/memory/src/recall.rs` — deep recall orchestration and scope bounds
- `crates/memory/src/observatory.rs` — evidence vault and rebuildable atlas
- `crates/retrieval/` — lexical, trigram, optional vector, and index recovery
- `crates/knowledge/` — profile knowledge ingestion and retrieval boundaries
- `crates/workspace/` — human-readable memory files and workspace ownership
- `crates/local-runtime/src/lib.rs` — agent memory tools and turn integration
- `crates/protocol/src/lib.rs` — client memory query commands and projections
- `apps/agent-tui/` and `apps/agent-web/` — user-facing memory search
- `spec/keith-agent/spec.kvx` — memory requirements and current qualification
  status
