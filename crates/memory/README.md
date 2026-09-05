# Canonical memory intake

`MemoryService::ingest_committed_entry` accepts an opaque receipt from the real
session writer. It makes current ingress available without advancing replay past
older history. `ingest_committed_page` accepts a profile-checked, checksum-verified
append-order page from `SessionStore`; its input cursor must match the saved
cursor. Production has no raw `SessionEntry` queue that can attest arbitrary text.

The existing memory vault owns evidence and source commitments. A
`SourceCommitted` event records a session/entry/checksum receipt and creates no
claim, atlas anchor, or independent supporting episode. Commitment conflicts fail
closed. A completed canonical append remains committed if atlas replacement fails;
`repair_ingestion_projection` and subsequent reads retry the derived projection.

`.keith/memory-source-cursors.json` is a disposable checkpoint. Intake commits the
vault before saving cursor progress. A crash or failed checkpoint write leaves
replay safe to repeat. Pending final/compaction text and delayed lineage are
checked against canonical vault commitments before reuse; a recomputed checksum
inside this disposable file cannot attest different text. Corrupt JSON is
quarantined for canonical replay. Invalid profile, version, or committed-source
identity is an explicit error.

Complete `AssistantFinal` bytes wait for their matching terminal and referenced
outbox/snapshot records. A compaction summary waits for its matching checkpoint.
The summary and checkpoint are generated representations with the same original
context roots. Copying the checkpoint retains those roots. Final candidates do
not enter the evidence vault. These checks preserve intake eligibility; they do
not certify all turn finalization transactions or delivery behavior.

`EvidenceCausalMetadata.source_roots` and `derived_from` describe context lineage,
not positive support for every claim in a summary. Exact quotation retains source
authority; rewritten memory is `DerivedInference`. Memory-tool results that carry
canonical records are transclusions and acquire no fresh observation root.
Unavailable, deleted, conflicting, unsupported, and limited origins remain
explicit gaps. Late original sources can repair copied/summary lineage through
append-only annotations, without upgrading an already generated record. Existing
roots and effective intervals survive repair.

Legacy vault bytes remain unchanged. Canonical replay recognizes the older
compaction-summary identity and annotates that record in place. Legacy generated
memory-tool observations can be conservatively downgraded through an annotation.
Legacy synthetic digest arrays are not accepted as exact citations. Consolidation
waits for actual canonical source checksums instead of manufacturing digests;
daily and unresolved records retain the checkpoint's source range. Historical
compaction drafts that were never durably stored cannot be reconstructed exactly.

The checkpoint is capped at 8 MiB and 4,096 sessions, with at most 1,024 pending
entries per session. Each intake retries at most 128 delayed lineage entries with
a durable rotation cursor. Root/reference/gap sets are capped at 256 and expose
limit gaps. Oversized evidence projections report a consumed gap so later valid
entries can progress. The session reader independently bounds source pages.
These bounds do **not** bound existing full manifest discovery or whole-vault
refresh/projection work.

A process-shared vault lock refreshes canonical state before reads and writes.
Short contention waits up to two seconds, then reports `Busy`; no stale snapshot
is substituted. Exact create/correct/forget citations and targets are checked
again while that lock is held. The separate ingestion lock serializes checkpoint
progress and can report `IngestionBusy`. Source commitments remain internal vault
metadata and must participate in future retention/invalidation work.

Focused validation uses real temporary stores:

```sh
CARGO_TARGET_DIR=/tmp/keith-causal-memory21 CARGO_INCREMENTAL=0 cargo test -p keith-memory --locked
CARGO_TARGET_DIR=/tmp/keith-causal-memory21 CARGO_INCREMENTAL=0 cargo clippy -p keith-memory --all-targets --no-deps --locked -- -D warnings
```

The real daemon/worker/provider/Web qualification is separate from these crate
checks and is owned by the active causal-intelligence qualification suite.

## Exact operational bindings

`memory_create_binding` and `memory_correct_binding` append one
`binding_associated` vault event containing both the owning durable memory and
its entity/property association. A bound correction supersedes the prior owner
in that same event. The existing vault lock and hash chain govern these writes;
a torn event cannot leave half a pair committed. Entity and property indexes are
rebuilt in memory from those events. There is no separate binding authority or
vector-based entity identity.

Each reference points to the original cited source, its content digest, an exact
UTF-8 byte span, the quoted value digest, and the association's event revision.
The owning memory is referenced separately. Rewritten memory stays inferential,
while the original source authority survives. The entity/property association is
explicitly `inferred`; resolving a unique attributed value does not establish
that it is true in the external world or authorize an action.

`lookup_binding` resolves current, missing, stale, or conflicting state by exact
workspace/entity/property identity. Explicit corrections form bounded chains;
missing links, cycles, unbound corrections, disputes, freshness failures, and
unknown effective times remain visible gaps. An explicitly future-dated
correction becomes current at its effective start. An effective-time historical
query needs a known interval; `recorded_as_of` instead replays what the vault knew
at an earlier revision. Current deletion and sensitivity restrictions still
apply to historical reads. Deleting either the quoted source or owning memory
invalidates that binding. Correcting either through an ordinary unbound memory
write creates an explicit `UnboundCorrection` gap. Bound repair follows the
canonical owner supersession chain, requires the request to name its exact
current owner, and atomically supersedes that owner while linking the prior
binding. It never chooses a different entity or overwrites an unrelated
conflicting association. Explicitly deleted or expired alternatives do not
compete with another still-current permitted binding.

For these two bound write APIs, `MemoryError::Binding`, `InvalidEvidenceQuote`,
`InvalidRequest`, and `EmptyText` guarantee that the requested write did not
append; correction's `MissingRecord` has the same guarantee. `OwnerMismatch`
specifically means correction's `evidence_id` did not name the current owning
memory. Use `owner_memory_id` from lookup or `evidence.id` from a write receipt;
the unchanged `expected_binding.evidence_id` identifies the original quoted
source, and the replacement citation belongs in `source`. Stale references remain
separate errors. Storage errors, `Changed`, and response serialization errors
must retain uncertain commit status. This contract does not classify errors from
unbound memory APIs or authorize blindly retrying rejected arguments.

`resolve_required_bindings` reads up to 128 distinct current requirements from
one canonical snapshot, independent of lexical or semantic top-K. It rejects
historical queries for action admission. `validate_binding_use` checks the
current reference, exact proposed value, target kind, allowed source authorities,
sensitivity, freshness, and whether inferred associations are permitted. The
runtime owns actual task scope, adapter target slots, and admission authority;
this API does not supply a task controller or universal confirmation step.

`binding_alias_candidates` returns only registered aliases appearing as exact,
case-sensitive tokens in bounded context. Repeated names do not merge entities;
ambiguous names and result truncation remain explicit. It includes no bound
values and filters candidates by current source/owner privacy. Bounds are 128
returned candidates, 64 KiB context, 256 bytes per alias, 16 KiB per quoted value,
and 4096 records per correction closure. Existing whole-vault refresh and
snapshot costs remain; bounded result counts do not claim bounded archive I/O.

Memory tool results and commitment create/get descriptions remain inferential
when re-ingested. A model-authored commitment description cannot acquire
`ToolObserved` authority by passing through a successful tool call. Canonical
commitment existence and lifecycle state remain available through their typed
owner; this content-classification rule does not replace that state authority.

Lexical search scores all visible candidates before applying its existing rank
and limit. It clones evidence and computes matched nodes/excerpts only for the
returned records. Coverage still reports the full inspected and matched set;
this optimization does not change ranking or claim bounded archive scanning.
