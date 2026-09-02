# Skills

Keith skills are declarative, versioned procedures selected into a turn when
their triggers match the task and their required tools are available. A skill
describes inputs, steps, validation, known failures, stop conditions, and
platform support. It guides the model; it does not itself execute code or grant
tools, credentials, filesystem access, or approval.

The registry tracks provenance and precedence across built-in, global, project,
and profile scopes. Mutable profile skills have durable history, conflict-safe
updates, enable/disable state, rollback, and deletion.

## Current status

The registry, relevance selection, lifecycle history, learned-skill candidate
flow, and task-recipe publication are implemented with focused in-crate tests.
Keith ships one built-in `repository-awareness` skill. There is no separate
skills qualification manifest or marketplace/client installation flow today.

Two current integration limitations matter:

- `./keith scaffold skill` emits YAML-style `---` front matter containing only
  `name` and `description`, while the runtime registry requires TOML `+++`
  front matter and the complete `SkillManifest`. The scaffolded file is not
  loadable by `SkillRegistry` without manual conversion.
- `AgentProfile.enabled_skills` is defined and validated, but the local runtime's
  turn selection does not currently intersect discovery with that list. The
  registry's own enable/disable ledger is enforced; profile configuration alone
  is not a reliable activation switch yet.

## Author a runtime-compatible skill

Create `<root>/<skill-id>/SKILL.md`. The directory name must exactly match the
manifest ID. Use TOML front matter between `+++` delimiters:

```markdown
+++
id = "release-check"
version = "1.0.0"
description = "Validate a Keith release candidate before publication"
triggers = ["release", "publish", "release candidate"]
inputs = ["requested artifact", "target platform"]
steps = ["inspect provenance", "run focused checks", "run packaged smoke test"]
required_tools = ["filesystem"]
validation = ["artifact digest recorded", "smoke path passed"]
known_failures = ["signing authority unavailable", "target platform unavailable"]
stop_conditions = ["requested publication authority is missing", "artifact digest changed"]
platforms = ["linux", "macos", "windows"]
+++
# Release check

Inspect the exact candidate and its provenance. Run only the checks authorized
for this release, preserve the observed results, and stop before publication
unless release authority is explicit.
```

Every array except `platforms` must be non-empty. IDs and required tool names
are bounded to ASCII letters, digits, `-`, and `_`; the package has a 256 KiB
default size ceiling. The body after front matter must be non-empty.

`./keith scaffold skill NAME [DIR]` can still create the directory skeleton,
`references/`, and `scripts/`, but replace its generated `SKILL.md` with the
runtime-compatible form above until the scaffold and registry formats converge.
The registry currently reads only `SKILL.md`; files in `references/` and
`scripts/` are author resources and are not automatically loaded or executed by
the skill subsystem.

## Discovery roots and precedence

For each profile, local runtime opens these roots:

| Scope | Current root | Mutability |
| --- | --- | --- |
| Built-in | distribution `packaging/builtins/skills` | immutable through registry |
| Global | `$KEITH_DATA_ROOT/skills/global` | discovered from operator-managed files |
| Project | `<workspace>/.agents/skills` | discovered from project files |
| Profile | `<workspace>/.keith/skills` | installed and versioned by registry |

When multiple scopes contain the same ID, precedence is profile, project,
global, then built-in. Within one scope, lexical source path breaks a tie. A
disabled ID is excluded regardless of the winning scope. Discovery rejects
symlink directories/files, mismatched directory IDs, oversized packages,
malformed front matter, and a scope exceeding its package count limit.

Project skills are repository content and can be reviewed with the code.
Profile skills are personal-workspace state. Do not copy secrets or private
session data into either one.

## Selection and turn injection

```text
discover four roots
  -> validate manifest, path, digest, and enabled state
  -> choose highest-precedence package per ID
  -> filter by current platform and ready tools
  -> score ID, trigger, and description against the task
  -> enforce max skill count and serialized prompt-byte budget
  -> inject selected package as developer-policy context
```

The local runtime currently selects at most eight skills and 64 KiB of skill
context per turn. Required tools are checked against the profile's ready tool
set. A skill with no positive relevance score, a platform mismatch, missing
tools, or exhausted context budget is excluded with a reason.

Selected skill content enters the model context as policy guidance, but it does
not bypass the ToolManager. If a skill says to use a denied tool, change a
credential, deploy, or ignore approval, deterministic runtime policy still
wins.

## Profile skill lifecycle

The `skill_manage` runtime tool supports:

- `install` with the complete skill source;
- `enable` by ID;
- `disable` by ID; and
- `delete` by ID.

It is a state-changing tool and remains subject to normal tool policy. Registry
APIs additionally expose digest-checked update, inspection, and rollback to a
historical revision.

Profile writes use the versioned personal workspace and expected content token.
An external edit between read and write produces a conflict instead of a silent
overwrite. The ledger records install/update/rollback/enable/disable/delete,
revision, digest, version, backup path, and timestamp. Deletion removes only the
mutable profile package and retains lifecycle history; built-in-only packages
cannot be deleted.

## Learned skills and task recipes

`SkillSynthesisService` can build an inactive candidate from repeated committed
workflow outcomes. The default policy requires at least three consistent,
successful outcomes, a measurable improvement, clean-workspace applicability,
regression cases, available non-blocked tools, no sensitive markers, and human
review. Candidates move through awaiting review, ready, rejected, activated,
and rolled-back states.

Activation rechecks the candidate digest and installs it through the same skill
registry. Rollback removes only the exact unchanged activated candidate and
retains its history. Candidate state lives under
`<workspace>/.keith/.keith/skill-candidates` relative to the registry's personal
workspace root as currently constructed.

Task recipes provide another publication route. A recorded procedure must have
accepted passing qualification before `RecipePublisher` renders a complete
`SkillManifest` and installs or digest-checks an update through `SkillRegistry`.
Recipe inputs, recovery branches, approval points, required tools, declared
checks, and platform list become the declarative skill package.

## Security boundaries

- Skill source is instruction content, not authority. Runtime code still
  enforces tool, profile, credential, network, approval, and process policy.
- Discovery refuses symlinks and directory/manifest ID mismatches. Profile edits
  use personal-workspace confinement, digests, revisions, and conflict handling.
- Provenance records the winning scope, origin, source path, content digest,
  installation time, and revision so a user can distinguish built-in, project,
  global, and personal instructions.
- Learned candidates reject committed failures, private-data flags, secret-like
  strings, blocked or unavailable tools, duplicate procedures, failed clean
  applicability, and regressions before review.
- A project skill is untrusted repository content and a global skill is
  operator-managed content. Neither should be allowed to reinterpret retrieved
  data or tool output as higher-priority policy.
- Scripts and references are not automatically executed. If another subsystem
  later exposes them, it must apply normal workspace and process authority.

## Failure and recovery

- Malformed, unsafe, oversized, or symlinked packages fail discovery explicitly;
  they are not partially loaded.
- A stale digest on update returns a conflict. The current package remains
  intact and can be inspected before retry.
- Disable is reversible. Rollback restores an immutable historical source into
  the profile workspace and creates a new lifecycle entry.
- Registry ledgers and history survive restart and reject a newer unsupported
  schema. External workspace changes are scanned before mutation.
- A deleted profile package can reveal a lower-precedence package with the same
  ID on later discovery; inspect candidates and provenance when this matters.
- Learned candidates remain inactive until validation and configured review are
  complete. Rejected, changed, non-improving, or regression-producing candidates
  cannot activate.

## Current limitations

- The scaffold and runtime manifest formats are inconsistent; use the TOML
  `+++` format documented here.
- Profile `enabled_skills` is not yet wired into runtime selection. Use registry
  disable for an effective stop, and verify the actual selected context.
- Only `SKILL.md` is loaded. Reference and script directories have no automatic
  runtime semantics.
- Skill relevance is deterministic lexical scoring with optional manifest
  descriptions/triggers, not semantic certification that the procedure is
  correct for a task.
- There is no public marketplace, remote installer, signature system, or
  standalone skills qualification manifest.

## Validate changes

Focused registry and synthesis checks:

```bash
cargo test -p keith-skills --locked
cargo clippy -p keith-skills --all-targets --no-deps --locked -- -D warnings
```

Changes to turn selection or `skill_manage` also require focused
`keith-local-runtime` tests. Task-recipe publication changes require
`keith-task-recipe` tests. Use an external disposable Cargo target.

Cover all four scopes and precedence, same-ID candidates, platform/tool/relevance
filters, count/byte budgets, malformed/symlink/oversize refusal, install/update
conflict, enable/disable, rollback, delete, restart, provenance, secret rejection,
review, regression, and exact candidate rollback. A prompt snapshot alone does
not prove a skill's requested external action succeeded.

## Key source locations

- `crates/skills/src/lib.rs` — manifest, discovery, selection, provenance, and
  profile lifecycle
- `crates/skills/src/synthesis.rs` — learned-skill candidate validation, review,
  activation, and rollback
- `crates/local-runtime/src/lib.rs` — roots, turn selection, prompt injection,
  and `skill_manage`
- `crates/task-recipe/src/publication.rs` — qualified recipe-to-skill rendering
- `crates/task-recipe/src/teaching.rs` — recipe publication orchestration
- `packaging/builtins/skills/repository-awareness/SKILL.md` — valid built-in example
- `crates/configuration/src/lib.rs` — profile skill list configuration
- `keith` — current scaffold implementation and known format mismatch
