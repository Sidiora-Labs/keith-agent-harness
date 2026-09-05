# Local runtime

This crate composes the canonical stores, model loop, and tool adapters used by
Keith's daemon and worker. Clients do not own a second agent or memory store.

## Exact object bindings

`memory_create` accepts an optional `binding` containing an entity identity or
new alias, property, target kind, and an exact quote from the cited source.
`memory_correct` accepts a correction draft together with `expected_binding`;
its `evidence_id` identifies the owning memory being corrected. The memory
service atomically records the memory and its binding. An inferred association
does not turn the original source into verified external truth.

For corrections, use `owner_memory_id` from binding lookup or `evidence.id`
from the write receipt. The nested `expected_binding.evidence_id` identifies
the original quoted source; `source_entry_id` supplies the new correction's
citation. Wrong-owner, invalid quote, and other proven pre-append bound-write
rejections report `NotCommitted` with automatic retry disabled. Storage errors
and failures after committing remain conservative; the runtime does not infer
non-commitment from a generic error. Hosted qualification allows an inspected,
corrected attempt only when failed attempts report no committed effect and the
canonical history contains exactly the expected successful binding transitions.

`memory_context.required_bindings` resolves canonical keys independently from
optional ranked recall. `read` and `web_fetch` accept `object_binding` to select
one object when several required objects use the same adapter slot. The runtime
derives scope from the actual accepted action and canonical goal. It refreshes
registered exact alias candidates before each tool admission, including after
compaction and mid-turn memory capture. Required keys accumulate in the session
owner's durable history; omitting an argument cannot erase them.

The held session writer records a frozen admission before dispatch. The runtime
then checks the exact source, binding revision, permitted provenance and target
again immediately before calling the real tool manager. A changed binding or
wrong target returns `NotStarted`. Unavailable canonical alias context cannot
authorize an opaque fallback. A model-generated summary cannot grant tool or
profile authority.

Current inspectable slots are `read.path` and `web_fetch.url`. With outstanding
requirements, native write/list/search/review routes and shell, browser, kernel,
MCP and plugin routes are explicitly unsupported. This guard also restricts a
bound-input-to-new-output workflow: broader parity requires host-defined target
roles, not a model-selected bypass flag. Binding validation does not provide
universal transactional execution or exactly-once effects.

Alias candidates and requirements are bounded at 128. Alias task text is bounded
at 64 KiB. The initial binding context omits detailed resolutions above 128 KiB
and directs the model to exact lookup; durable requirements remain enforced.
Canonical memory refresh and session requirement history retain their existing
full-scan costs. The alias registry is an explicit, case-sensitive candidate
lookup, not semantic entity resolution.

`commitment_get` reads a profile-scoped canonical commitment and revision by
exact ID. Initial current-session commitment context is bounded to 128 records
and 64 KiB, reports truncation, and identifies this exact lookup tool. It does
not replace the commitment owner's state machine.

`bindings_tests` exercises real stores, workspace tools, admission failures,
correction between admission and dispatch, ambiguity, source freshness, and the
actual HTTP provider/loop path. The hosted three-replicate journey lives in
`tests/causal-intelligence/test_runtime_bindings.py`; task completion additionally
requires its source-bound verifier.

The crowded runtime case uses 10,000 real session source commits, then one
canonical source/evidence batch to prepare the unrelated archive. It captures
and corrects the target through the production memory tools, starts a fresh
session, checks the actual provider request and frozen admission, and executes
the real workspace read. This fixture qualifies retrieval and dispatch under
crowding; automatic intake throughput remains a separate concern.
