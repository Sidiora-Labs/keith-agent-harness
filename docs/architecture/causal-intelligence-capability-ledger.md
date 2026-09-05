# Causal intelligence: Gideon capability ledger

Task: `keith-causal-intelligence/1.4` · inspected 2026-09-05.

This is a **source inventory and extraction decision**, not a claim that any
donor capability has been imported or qualified in Keith. Cheap means low
implementation effort: useful behavior gained per integration boundary changed.
API/model prices do not rank these candidates.

The authoritative machine-readable snapshot is
[donor-inventory.json](../../tests/causal-intelligence/donor-inventory.json).
It enumerates every discovered item, its exact source path, SHA-256, byte count,
family, and disposition. Family records supply ownership, existing equivalent,
missing behavior, changed imports, dependencies, platform limits, readiness,
required proof, and the next task or explicit follow-on destination. File and
family records together form one candidate record; an app manifest is an
inventory item, not proof that the app works.

## Source identity and completeness

| Source | Identity and limits |
| --- | --- |
| `/root/Gideon/Gideon-ai` | Git `70b043fb8f9b653a25b4e268fdea3cb8047c0ba2`, plus per-file hashes. The selected document, skill, projection, reader, workflow, and recipe paths have no tracked modifications; unrelated dirty work is excluded. |
| `/root/Gideon/Gideon-ai-apps` | No independently resolvable HEAD. Git resolves to `/root/Gideon`, whose HEAD was unborn at inspection. Per-file SHA-256 is the source pin; no commit identity is invented. |
| License | Both root `LICENSE` files carry MIT and `Copyright (c) 2026 Gideon contributors`. Root and available per-app license files are hashed. Preserve notices on import; dependencies and model weights require their own license review. |

| Collection | Exact discovery pattern, relative to source root | Count |
| --- | --- | ---: |
| Native apps | `src/gideon/apps/native/*/app.json` | 30 |
| External apps | `*/app.json` | 52 |
| Bundled skills | `src/gideon/skills/bundled/*/SKILL.md` | 17 |
| Bundled workflows | `src/gideon/workflows/bundled/*/workflow.json` | 29 |
| Source recipes | `src/gideon/knowledge/sources/recipes/*.json` | 7 |
| Document Python modules | `src/gideon/documents/**/*.py` | 17 |

The snapshot pins these **152 discovered items** and **16 selected support
modules**, with license and dependency-lock records separately. Historical
research counts of skills/workflows were approximate. The tests compare the
recorded sets with the actual trees; equal counts alone are insufficient.

All dispositions in this initial snapshot are `existing_equivalent`,
`follow_on_adapter`, or `stateful_product_migration`. `existing_equivalent`
means a Keith owner and corresponding source behavior were found; every such
family still has `runtime_qualified: false`. No item is `implemented_here`.
Task 7.6 must reconcile this snapshot with subsequently observed user behavior.

## First batch and integration order

1. Four curated skills whose basic required tools already exist.
2. One extracted XLSX writer package through the existing Python and artifact
   boundaries, then DOCX/PPTX/PDF through the same host path.
3. One log projection that exposes errors hidden in long output while preserving
   the complete raw artifact.
4. One ordinary Web journey that downloads and reopens the generated document,
   including after restart.

These select content, pure functions, and one shared host adaptation. They do
not require a Gideon gateway, SDK emulation layer, marketplace, memory/session
store, approval controller, updater, shared-venv installer, or second agent loop.
The new installed capability must participate in Keith's world/capability
version and admission rules.

### Curated skills: task 3.5

Sources are `src/gideon/skills/bundled/<id>/SKILL.md` in the pinned core checkout.
Destinations are `packaging/builtins/skills/<id>/SKILL.md`.

| Selected skill | Actual Keith required tools | Adaptation and proof |
| --- | --- | --- |
| `document-authoring` | `write` | Keep audience, evidence, structure, and limitations. Writing Markdown is available; rendering a binary document is only advertised when that format's adapter is ready. |
| `knowledge-grounding` | `memory_search`, `memory_get` | Replace `knowledge_*` references. Keep source authority and generated/inferred labels. Retention follows Keith policy; generated summaries cannot become observed facts. |
| `check-work` | `read`, `bash` | Check concrete claims with real commands. Report unavailable proof honestly; do not force redundant checks or confuse focused proof with release qualification. |
| `research-campaign` | `read`, `web_fetch` | Preserve scoped questions, primary evidence, contradictions, and stopping criteria. Use available sources without pretending a search provider exists. Do not force clarification when the task already supplies its scope. |

Convert YAML `---` into TOML `+++` with complete `SkillManifest` fields:
`id`, `version`, `description`, `triggers`, `inputs`, `steps`, `required_tools`,
`validation`, `known_failures`, `stop_conditions`, `platforms`. The arrays other
than platforms must be nonempty. Use real tool IDs, not the illustrative
`filesystem` name or Gideon's `knowledge_*` tools. The current registry loads
`SKILL.md`; it does not automatically load/execute references and scripts.

The real turn must demonstrate selection and exclusion for disabled,
malformed, unavailable, and unsupported skills. Repair the scaffold/runtime
format mismatch and profile `enabled_skills` selection at their actual owners.
See [Keith skills](../features/skills.md) and `crates/skills/src/lib.rs`.

`web-verify` is a useful next skill. Its donor procedure requires clicks,
screenshots, console/network inspection, and bundle identity checks. Keith's
current `BrowserTool` in `crates/local-runtime/src/lib.rs` only navigates and
returns a semantic observation. Tool name availability does not establish those
interaction capabilities; admit the full procedure only when the required
interaction path is ready.

### Pure document extraction: tasks 3.4 and 4.6

The first package consists of these core donor paths:

| Path below `src/gideon/documents/` | Role |
| --- | --- |
| `model.py` | Shared dataclasses; standard-library imports only |
| `model_json.py` | Strict model/scalar decoding reused by sheet decoding |
| `sheet_json.py` | Typed workbook JSON codec |
| `registry.py` | Format-to-writer registration boundary |
| `writers/xlsx_writer.py` | `SheetModel` to XLSX bytes using openpyxl |

These five files total 39,787 source bytes. This is source footprint, not an
installed package or platform size estimate. Proposed destination:
`packaging/python/keith_documents/`, which is not created by this task.

Rename package-local imports and use a thin `__init__.py`. The original package
initializer imports all codecs. Do not pull `model_codec.py`, parsers,
`from_markup.py`, Gideon security/web modules, or a full app SDK into the first
writer. Restrict registry imports to packaged writers; distinguish missing
dependencies from implementation errors instead of silently swallowing both.

| Dependency | Inspected `uv.lock` version | First use |
| --- | --- | --- |
| openpyxl | 3.1.5 | XLSX |
| et-xmlfile | 2.0.0 | openpyxl dependency |
| python-docx | 1.2.0 | DOCX |
| python-pptx | 1.0.2 | PPTX |
| reportlab | 4.5.1 | PDF |
| pdfplumber | 0.11.10 | PDF reading/inspection |
| lxml | 6.1.1 | Relevant format-library dependency |
| pillow | 12.3.0 | Relevant image/format-library dependency |

Only package the chosen dependency closure. These are inspected donor pins,
not a statement that every package supports every Keith platform. Core requires
Python 3.12+. Verify selected wheel availability, actual package footprint,
dependency notices, and rendering behavior in each claimed packaged target.

Preserve numeric/boolean/string cell types and explicit formula cells. A literal
beginning with `=` must remain literal. The writer does not calculate formulas.
Format support is scoped to the modeled features; unsupported round-trip
content must receive a loss report rather than a claim of losslessness.

### Existing host path and concrete gaps

| Boundary | Current owner | Required adaptation |
| --- | --- | --- |
| Admission | `crates/tool-core`; `LocalRuntime::tool_manager` | Typed capability entry point, readiness, and existing effect/authority semantics |
| Python process | `crates/kernel-broker`; `KernelTool` in local runtime | Packaged dependency/interpreter identity, bounded inputs/output, cancellation and cleanup |
| Guest protocol | `crates/kernel-protocol::BridgeOperation` | Existing `CreateArtifact` accepts only `text`; add a bounded binary result route without guest-owned paths or profile IDs |
| Durable artifacts | `crates/artifacts::ArtifactService` | Reuse binary create/download/export, digest checks, limits, and profile/tree scope |
| User delivery | `apps/agent-web` and shared daemon projections | Complete and prove ordinary generated-document download; an `artifact_id` alone is insufficient |

`KernelTool::python()` currently selects `/usr/bin/python3` or
`/usr/local/bin/python3`. Installing dependencies in an unrelated virtualenv
does not make the selected runtime ready. The default isolated kernel requests
network denial; the explicit trusted fallback has different authority. Do not
claim OS isolation from a subprocess or a package manifest alone.

`RuntimeBridge::create_artifact` currently stores `text.as_bytes()`.
`ArtifactService` already supports binary bytes, so the missing seam is typed
binary transfer/commitment, not storage replacement. The guest must not select
another profile's artifact scope or become a completion authority.

The inspected Web command response returns an artifact ID, and the UI has
"Prepare export". No ordinary generated-document download route was found in
the inspected Web server/contracts. Task 5.4 must establish the real route and
its authentication rather than infer it from artifact-store tests.

### Useful output projection: task 4.7

Keith's `crates/artifacts/src/lib.rs::preview` emits a bounded UTF-8 prefix or
binary hex. Gideon's `tool_providers/projection.py::_project_log` retains the
head, middle error/warning lines, and tail. This is an actual missing preview
behavior worth comparing on real long output.

The entire donor module is not pure: rule loading, project context, savings
accounting, and `project_and_retain` call storage/configuration. Extract only
the bounded shaping function and necessary truncation helper. Keep raw-output
storage, scope, and retrieval in Keith; never import `result_store.py` as a
second store. A heuristic preview is not sufficient evidence coverage. Preserve
exact retrieval of required lines and explicit truncation metadata.

## Follow-on useful capabilities

| Capability | Representative pinned donor | Missing behavior / owner / proof |
| --- | --- | --- |
| Rich reader, task 5.5 | Core `documents/docx_parser.py`, then XLSX/PPTX parsers and `knowledge/readers.py` | Host-provided bounded artifact bytes, source coordinates and loss reports; kernel/artifact/memory owners. Actual malformed/oversized document and successful upload/read journeys. Existing attachment expansion includes text/JSON, not rich document extraction. |
| Search, task 5.6 | Apps `brave-search/provider.py`; Tavily as comparison | Normalize bounded URLs/snippets/recency; replace SDK search/net and environment credentials with web-owned contracts and credential references. Real authenticated search followed by source readback. Brave is a small single-GET donor, selected for integration effort. |
| Source monitoring, task 5.7 | Core `knowledge/sources/recipes/github-releases.json` | Atom feed over Keith fetch/scheduler; persistent source identity, seeded first poll, item cursor, duplicate suppression, cancellation and restart. Translate content without `triggers/web_poll.py`'s controller/store. |
| Transcription, task 6.4 | Apps `faster-whisper/provider.py` | Bounded audio worker and model-ready state; dependencies include faster-whisper/CTranslate2 and model assets. Real recording/transcript and cancellation, not a manifest-only pass. |
| Speech synthesis, task 6.4 | Apps `piper-tts/provider.py` | Replace Gideon model directories, binary discovery and sandbox wrapper. Voice/model licensing is separate from donor MIT. Real WAV output, model absence, timeout and cleanup. |
| Inbox, task 6.5 | Apps `mail-inbox/mail_inbox_runtime/{mime,imap_client,smtp_client}.py`; email-channel equivalents | Extend Keith account/channel/delivery owners. MIME/IMAP adds value beyond Keith's existing provider HTTP/webhook EmailAdapter. Replace temporary-file extraction and SDK calls with host-owned artifact access; preserve untrusted origin. |

Speech dependencies are not free installation assumptions: faster-whisper,
piper-tts/huggingface-hub, diarization ONNX/sherpa/soundfile/numpy, and
pyannote/torch are recorded from app manifests. Model downloads, native wheels,
memory, audio duration, cancellation, and supported platforms need their own
bounded proof. Diarization follows the basic audio path.

Keith already has Discord, Slack, Telegram, WhatsApp, Teams, Google Chat, Email,
and Matrix adapters in `crates/channel-adapters`. Retain their profile credentials,
conversation IDs, receipts, reconnect cursors, and possible-duplicate states.
Neither reusing a transport nor parsing an email grants send authority. Live
qualification requires an authorized disposable recipient; absent credentials
remain unavailable.

## Complete remaining breadth dispositions

The JSON contains every item's exact path/hash; the following groups account
for all **52 external manifests** without treating package counts as parity.

| Family | Apps | Destination |
| --- | --- | --- |
| Models (16) | alibaba-models, anthropic-compatible, anthropic-models, bedrock-models, claude-subscription, deepseek-models, google-models, groq-models, meta-muse-spark, mistral-models, ollama-models, openai-compatible, openai-models, openrouter-models, together-models, vllm-models | Compare vendor behavior with provider-adapters; port missing behavior only |
| Search (7) | brave-search, duckduckgo-search, exa-search, perplexity-search, searxng-search, tavily-search, wikipedia-search | First Brave; remaining provider adapter follow-ons |
| Tools (2) | web-tools, openai-tools | Compare against Keith web/provider tools |
| CLI agents (4) | claude-code-agent, codex-agent, gemini-cli-agent, kiro-cli-agent | Existing subagent/ACP execution boundary; separate transport review |
| Remote actions/image (2) | a2a-action, fal-image | Explicit adapter follow-ons |
| Speech (4) | faster-whisper, piper-tts, diarization-onnx, diarization-pyannote | Audio workers with independent model/platform readiness |
| Embeddings (1) | sentence-transformers | Optional trained encoder adapter; no donor FAISS/store import |
| Sync (4) | dir-sync, git-sync, rsync-sync, s3-sync | Separate stateful conflict/deletion/recovery projects |
| Channels/inbox (5) | discord-channel, email-channel, mail-inbox, slack-channel, telegram-channel | Prefer existing Keith contracts; select IMAP/MIME gaps |
| MCP/distribution/automation/webhook (4) | mcp-tools, skills-sh, shared-automations, webhook-action | Existing MCP review, deferred skill distribution, recipe translation, and webhook adapter respectively |
| Product apps (2) | minutes, growth | Separate product integration over Keith-owned data |
| Device (1) | menu-bar-companion | Separate client over daemon projections |

The **30 native manifests** consist of nine action adapters, six `native-*`
state/provider modules, fourteen `gideon-*` tool/product bundles, and
`filesystem-inbox`. They map to existing Keith tool/domain owners or explicit
ingress follow-on review. In particular, `native-vector-memory` and
`gideon-memory` must not introduce another memory authority.

All **17 skills** have explicit destinations:

- Initial: document-authoring, knowledge-grounding, check-work, research-campaign.
- Content follow-ons: editorial-document, artifacts, infographic-syntax,
  visual-output, web-verify, grill.
- Translate against Keith lifecycle: task-and-project, delegation, best-of-n,
  loop-worker, memory-discipline.
- Replace with actual Keith product documentation: gideon-api, gideon-features.

All **29 workflows** are inventoried as content/composition donors, with their
actual JSON files pinned. Prioritize `deep-research`, `produce-and-audit`,
`self-qa`, and `trending-repo-digest` for later translation. Their node/tool names
depend on Gideon and cannot be installed unchanged. `optimize-harness` stays
behind Keith's independent evaluator and promotion boundaries. A full workflow
editor is a separate stateful product migration, not a document prerequisite.

The seven recipes are github-releases, github-trending, hacker-news,
pypi-releases, changelog-page, reddit-subreddit, and substack-newsletter.
The first-poll seed and meaningful item identity ideas in `triggers/web_poll.py`
are useful; its file-backed controller is not a Keith owner.

Minutes/Growth backends, workflow editor state, synchronization, and device
companions are deliberately separate product work. Reuse algorithms or UI
components only after mapping every durable record/API to Keith. They do not
silently become either completed work or prerequisites for the low-effort batch.

## Required proof and task 1.4 verification

The first complete imported capability journey must execute the actual packaged
Python through Keith admission, render typed spreadsheet input, transfer binary
bytes through the bounded host bridge, commit the scoped artifact, expose a
working download in the normal Web conversation, reopen and inspect content,
then download again after restart. Include explicit formulas versus literal
strings, dependency absence, malformed/oversized input, process failure,
cancellation/timeout, cross-profile denial, and cleanup. No Gideon process or
state directory may be required.

Task 1.4's narrower source-conformance checks are executable now:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tests/causal-intelligence -p test_donor_inventory.py -v
python3 scripts/qualification/causal_intelligence.py verify \
  --task 1.4 --spec spec/keith-causal-intelligence/spec.kvx
```

The ten tests compare actual discovery sets, hashes, manifest versions,
dependencies/permissions, license notices, selected import closure, skill/tool
mappings, and Keith owner entrypoints. Negative cases reject same-count source
substitution, duplicate entries, unsafe paths, and a runtime-qualification claim.
They parse source without importing donor code or installing dependencies.

The recorded source roots can be relocated with `KEITH_GIDEON_CORE_SOURCE` and
`KEITH_GIDEON_APPS_SOURCE`. Missing/changed sources fail the proof rather than
skip. Refresh pins only after reviewing the change. A passing source-conformance
result qualifies the inventory task alone; no imported runtime, package,
provider, browser, or release journey is established by it.
