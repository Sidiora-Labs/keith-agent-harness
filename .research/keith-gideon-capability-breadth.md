# Keith: Gideon breadth through low-effort reuse

Date: 2026-09-05

Status: research proposal. **Cheap means easy to implement: low engineering effort, few new moving parts, and substantial reuse.** Operating cost is not the selection criterion. No code or capabilities have been imported.

This extends the settled [causal-memory plan](keith-causal-memory-implementation-plan.md). New capabilities must fit that plan's world, evidence, and authority boundaries.

## Recommendation

Acquire Gideon's useful capabilities in the form closest to what Keith can already consume:

1. Adapt skills and reusable task instructions.
2. Extract relatively self-contained Python transformations.
3. Wrap selected provider adapters through existing Keith boundaries.
4. Translate useful workflow content onto Keith's existing runtime.
5. Treat stateful apps and full platform parity as later integration projects.

The main engineering shortcut is to reuse implementations in their current language where sensible. Keith already has a Python kernel broker, tool manager, MCP manager, skills, scheduler, and artifact facilities. A Rust rewrite of every document parser or provider adapter would increase the work without being necessary for the capability.

This is a capability acquisition effort. Replacing Keith's runtime with Gideon's gateway would add competing state and control owners and make the task much larger.

## What is available to draw from

The inspected checkout has **30 native app manifests and 52 external app manifests**, plus bundled skills, workflow definitions, source recipes, and document transforms. The external apps README says 47; the filesystem count is newer. Counts describe packages, not proven functionality or migration completion.

Sources: [Gideon core](/root/Gideon/Gideon-ai/README.md), [native manifests](/root/Gideon/Gideon-ai/src/gideon/apps/native), [external apps](/root/Gideon/Gideon-ai-apps).

The earlier broad hybrid migration study is historical planning context. Its referenced plan file is absent at the recorded path on this host. Its full-parity estimates should not be used to price these smaller capability slices.

## Rank by implementation effort

These are relative source-based estimates, not calendar commitments. A capability only counts when it is usable through Keith.

| Priority | Capability | Reuse path | Effort and main dependency |
|---|---|---|---|
| 1 | Better document authoring, research discipline, browser verification, and task procedures | Adapt selected Gideon SKILL.md bodies to Keith manifests and real tool names | **Low.** Mostly content/metadata adaptation where Keith already has the necessary tools. |
| 2 | DOCX, XLSX, PPTX, and PDF creation | Reuse Gideon's document/sheet/deck models and Python writers | **Low–medium after one shared execution/artifact path.** Pin dependencies and validate actual files. |
| 3 | Rich document reading | Extract appropriate reader/parser logic | **Medium.** More format, path, malformed-input, and source-location handling than rendering. |
| 4 | Better large-result previews | Adapt Gideon's log/diff/JSON/CSV projection functions | **Low–medium.** Keep raw-output storage and access in Keith; preserve required evidence. |
| 5 | Additional search and extraction providers | Adapt selected Gideon provider functions | **Medium.** Protocol logic is reusable; configuration, credentials, network access, result contracts, and UI readiness still need wiring. |
| 6 | Research/source-monitoring recipes | Translate source definitions and routine logic onto Keith's existing scheduling/fetch facilities | **Low for templates, medium for unattended execution.** Requires source cursors, ownership, duplicate suppression, and a complete user path. |
| 7 | Transcription and speech | Adapt existing model-provider bundles into bounded workers | **Medium.** Downloads, native dependencies, model readiness, platform support, and audio lifecycle dominate the work. |
| 8 | Inbox/channel capabilities | Reuse transport/parsing logic under Keith's account and delivery model | **Medium–high.** Authentication, threading, events, restart, and actual sends make this more than a wrapper. |
| 9 | Full meeting-minutes/Growth apps, workflow editor, sync, and device companions | Rebuild the user experience over Keith-owned records, reusing algorithms and components selectively | **High.** These depend on Gideon's product state, APIs, and lifecycle. |

Semantic-memory improvements remain in the already settled plan; importing Gideon's memory store is not a shortcut.

## The easiest real wins

### Skills: mostly adaptation, not infrastructure

Gideon's bundled skills include document authoring, research campaigns, knowledge grounding, work checks, browser verification, and visual output. Start with the ones whose behavior is supported by existing Keith tools.

There is a concrete format mismatch:
- Gideon skills use YAML-style `---` metadata.
- Keith's runtime requires TOML `+++` metadata and a complete `SkillManifest`.
- Keith's documented scaffold currently produces a different format from its registry.
- Keith's documented per-profile `enabled_skills` selection also needs verification; the registry's own enable/disable state is enforced.

Therefore this is not “copy the directory and call it done.” Build a small explicit conversion for a curated batch, map required tools, review references to Gideon-only APIs, and prove selection in a real turn. This does not require a new marketplace or a full app SDK.

A skill improves how Keith uses existing capabilities. It cannot create a missing renderer, search provider, or operating-system permission by describing one.

Sources: [Keith skills contract](../docs/features/skills.md), [Gideon bundled skills](/root/Gideon/Gideon-ai/src/gideon/skills/bundled), [document-authoring donor](/root/Gideon/Gideon-ai/src/gideon/skills/bundled/document-authoring/SKILL.md).

### Document writers: the best self-contained code donor

Gideon's document registry defines writers as functions from structured models to bytes. The relevant implementation primarily imports its document model, registry, codecs, and format libraries. It has a clearer extraction boundary than the gateway, workflow controller, or app UI.

The adaptation:
1. Extract the necessary document model/codecs/writers as a pinned package with donor provenance.
2. Replace package-local import paths without changing behavior unnecessarily.
3. Run through Keith's existing bounded Python execution facility.
4. Expose a small typed entry point for structured input and output format.
5. Let Keith validate and commit the resulting artifact and show a working download.

A single shared host adapter can support several formats. Each format still needs a genuine round-trip/content/render check. Start with the least-coupled writer, then add the rest.

Do not require a general Gideon SDK emulation layer or arbitrary third-party app installation to reach this result.

Sources: [document model](/root/Gideon/Gideon-ai/src/gideon/documents/model.py), [registry](/root/Gideon/Gideon-ai/src/gideon/documents/registry.py), [writers](/root/Gideon/Gideon-ai/src/gideon/documents/writers), [Keith Python kernel broker](../crates/kernel-broker/src/lib.rs).

### Reusable functions before complete subsystems

Other promising donors include output projection, markdown conversion, structured document codecs, source extraction recipes, and bounded ranking functions.

Classify imports before extraction:
- **Pure function/content:** adapt directly with focused behavior checks.
- **Function with storage/network dependency:** replace that boundary with a Keith-owned interface.
- **Controller or stateful product:** design a deliberate migration; do not disguise it as a helper.

For example, Gideon's output projectors are useful donors; its raw-result store should not become a second Keith store. Likewise, source recipes can be translated without importing the trigger controller.

Sources: [projection functions](/root/Gideon/Gideon-ai/src/gideon/tool_providers/projection.py), [source recipes](/root/Gideon/Gideon-ai/src/gideon/knowledge/sources/recipes).

## One shared adaptation boundary

For the first extracted package, prefer Keith's existing Python kernel/tool/artifact route. Add only the missing typed adapter and packaging required by that capability.

Use MCP when its standardized tool surface or an existing server materially reduces work. Do not make a general MCP administration product a prerequisite for a pure document function. Keith's MCP library supports programmatic configuration, but its documentation records missing general operator setup; that work must be counted if this route is selected.

```text
Existing Keith turn
    -> existing tool admission
    -> bounded Python capability function
    -> structured result / output artifact
    -> Keith-owned persistence and user-visible result
```

The shared adapter needs bounded inputs, profile-scoped artifact access, cancellation/timeouts, dependency/version identity, and actual result reporting. These should reuse existing contracts and runners. The worker does not open memory/session stores, own approvals, or declare the overall task complete.

Network connectors are a separate boundary from pure transforms. Adapt credential and HTTP access through Keith's existing facilities, or explicitly contain a connector that needs direct vendor-SDK networking. Do not present subprocess isolation or manifest declarations as stronger enforcement than they provide.

Sources: [tool manager](../crates/tool-core/src/lib.rs), [kernel broker](../crates/kernel-broker/src/lib.rs), [MCP limitations](../docs/features/mcp-servers.md), [Gideon app platform](/root/Gideon/Gideon-ai-apps/docs/platform-architecture.md).

## First batch

Recommended first deliverable:

1. **Three to five curated skills** covering document authoring, evidence-grounded research, and work/browser verification, limited to actual available tools.
2. **One extracted document package**, starting with a writer whose output can be inspected end to end, then extending to other formats.
3. **One useful projection function** if Keith's current large-output path lacks equivalent behavior.
4. **One representative user journey:** give Keith structured material, have it use the imported procedure, render a real document, return it through the normal UI, and reopen it after restart.

This batch tests all three practical reuse modes—content, Python functions, and runtime integration—without requiring the Gideon platform.

After that, choose the next batch by actual remaining adaptation effort and user value: document readers, an additional search adapter, source-monitoring recipes, or speech. Avoid importing capabilities that Keith already provides adequately.

## What makes a slice complete

For each candidate, record:
- the user-visible capability and exact donor files;
- existing Keith equivalent and the actual missing behavior;
- dependencies/imports that must change;
- owning Keith contract and minimal adapter;
- package size/platform requirements and donor/dependency provenance;
- a real success case plus relevant failure/restart coverage;
- what remains unported.

The implementation-effort metric is **useful behavior gained per integration boundary changed**. Track actual time/changed components after the first slice rather than inventing precise estimates from file counts.

Do not describe pure-copy reuse where there is a new store, controller, authentication scheme, UI state model, or installation lifecycle. Those are the expensive parts.

## Preserved constraints

- Keith remains one agent and one authority over durable truth.
- New installed capabilities participate in the agreed versioned-world model.
- Imported procedures cannot override evidence, permissions, or finalization.
- Reuse Python where it reduces implementation work; Rust remains responsible for its existing host/domain boundaries.
- Preserve Gideon's useful behavior without carrying over its live stores, gateway, shared-venv installer, approval system, updater, or separate agent loop.
- No runtime-cost optimization is required to justify these additions.
