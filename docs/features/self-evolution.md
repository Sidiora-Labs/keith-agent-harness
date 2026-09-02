# Self-evolution

Keith's self-evolution system is a controlled way to improve the harness around
the model: explicitly permitted Rust source that helps determine how Keith
observes, reasons, and acts. It can turn failure traces or measured improvement
opportunities into competing source changes, build them in isolation, evaluate
them on hidden cases, and promote a winner through a reversible canary.

This is more than rewriting a prompt, but it is not unrestricted
self-modification. The candidate cannot edit the evaluator, held-out tests,
security policy, approval rules, sandbox, promotion gate, rollback path, or
other compiled protected surfaces.

## What the system does

1. Records a bounded execution trace with inputs, decisions, tool events,
   outcomes, resource use, and redacted evidence.
2. Determines whether the evidence supports a harness-caused opportunity. A bad
   score or injected instruction is not enough by itself.
3. Produces a falsifiable hypothesis and one or more candidate patches against
   an explicit editable allowlist.
4. Applies each patch to a commit-exact shadow tree, never to the live checkout.
5. Runs format, lint, test, dependency, security, platform, and resource gates
   selected by the independent evaluation plan.
6. Compares candidates on held-out cases that the candidate process cannot
   inspect or modify, rejecting leakage, regressions, reward hacking, and budget
   violations.
7. Packages and signs the selected worker image, then starts a limited canary.
8. Observes the canary and either records it as known-good or restores the last
   known-good source and image.

The system keeps the proposal, evidence, approvals, build results, evaluation,
promotion, observation, and reversal in an append-only evolution ledger.

## Operating modes

Keith models three operation modes:

- **Advisory** — diagnose and prepare evidence without live promotion.
- **Shadow** — build and evaluate candidates in isolation; promotion requires
  the authority and approval attached to the operation.
- **Autonomous** — permit only the lowest-risk classified changes to proceed
  through the complete independent gate and reversible canary.

Autonomous does not mean unbounded. The guard classifies changes and applies
different consent policies. Some changes require owner approval and some are
refused in every mode.

Normal Web and terminal clients can inspect availability, mode, candidates,
metrics, ledger records, approvals, retries, promotion, and reversal where the
daemon grants the corresponding authority. A client cannot turn self-evolution
on for an installation: the daemon explicitly rejects the native `Enable`
command. Enablement is an installation-owner lifecycle decision.

The current repository does not document a supported public host command for
granting that enablement. Operators should therefore treat the feature as
disabled unless their packaged installation explicitly provisions owner
authority and reports it as available.

## Architecture and trust boundary

```text
bounded traces -> causal diagnosis -> hypotheses and candidates
                                           |
                                 commit-exact shadow trees
                                           |
                          independent build and held-out eval
                                           |
                           signed image + limited canary
                                           |
                       observe -> known-good or byte-exact revert

candidate-controlled: editable shadow source only
independent authority: policy, evaluator, tests, signing, promotion, rollback
```

`meta-harness` owns trace diagnosis, candidate isolation, dataset separation,
held-out evaluation, comparison, and the promotion state machine.
`self-evolution` owns installation enablement, the compiled guard, shadow-tree
staging, build isolation, signing, canary execution, watchdog, budgets, ledger,
and restoration. The daemon projects this state to clients but remains the
authority for mutations.

The evaluation data is divided into search, validation, and held-out sets. A
leakage canary detects candidate access to protected evaluation material. A
winner must remain on the permitted performance/resource frontier rather than
maximizing a single score at any cost.

## Protected authority

The compiled guard refuses edits to security- and governance-critical
surfaces. The protected set includes the evaluator and held-out material,
session and memory authority, sandbox and tool runner, build and release logic,
resource and budget policy, credentials, Cargo and toolchain control,
signing-key locations, build scripts, the evolution guard and ledger, and the
promotion and rollback implementation.

The proposal process is also denied direct shell, network, and arbitrary
filesystem authority. A candidate cannot:

- widen permissions, sandbox access, networking, or resource ceilings;
- read credentials or signing material;
- change its own evaluator, benchmarks, guard, budget, or approval class;
- install or update itself through an unreviewed channel;
- spawn another evolution process or recursively delegate evolution;
- create another daemon to escape the active lifecycle owner.

Text in a trace, tool result, page, or candidate explanation is evidence only.
Deterministic code decides whether a path, change class, evaluation, signature,
approval, or promotion is valid.

## Failure and recovery

- Unsupported, protected, stale, symlinked, or out-of-budget patches are
  rejected before qualification.
- Shadow trees are tied to an exact source revision and content manifest.
  Tampering quarantines the candidate.
- Build and evaluation processes run with bounded time, disk, memory, process,
  and network access. Required isolation fails closed if the host cannot provide
  it.
- A candidate that improves one measure while regressing a required measure is
  not promoted.
- Promotion requires the expected signed artifact and a successful canary; a
  source-only patch is not a deployable result.
- A failed canary or observation window triggers restoration to the recorded
  known-good source and image.
- Retry is state-machine controlled. An operator cannot skip failed phases by
  repeatedly pressing promote.
- Abandoned shadow trees are registered and reclaimed without recursively
  deleting an unverified path.

## Current limitations and status

- Self-evolution changes only allowlisted harness source. It does not alter the
  foundation model's weights and cannot change protected governance code.
- The quality of an improvement is bounded by the evaluation cases and metrics.
  Dataset separation, leakage checks, and canaries reduce risk but cannot prove
  universal improvement.
- Host support for strong process and filesystem isolation is required for the
  autonomous path. Advisory behavior remains possible when autonomous
  enablement is unavailable.
- Client controls may be visible while the daemon reports the feature disabled
  or the caller lacks promotion authority. That is an intentional state, not a
  UI-only toggle.
- The repository contains a focused deliberate-defect qualification journey
  covering candidate competition, hidden evaluation, signing, canary,
  observation, retry refusal, and byte-exact reversal. The active feature's
  release-wide security gate and full integration gate remain separate and
  incomplete; the focused evidence is not a claim of general release
  certification.

## Validate changes

Changes in this area require adversarial proof, not only a happy-path unit test:

- a deliberate harness defect with more than one candidate and a baseline;
- causal attribution that rejects non-harness failures and injected authority;
- protected-path, symlink, stale-source, shadow-tampering, recursion,
  credential, network, and resource-bound attacks;
- strict separation of search, validation, and held-out datasets;
- leakage-canary, regression, reward-hacking, and Pareto-selection behavior;
- actual formatting, compilation, lint, focused tests, dependency, security,
  and platform gates inside the intended sandbox;
- signature verification, limited canary, observation, known-good recording,
  crash recovery, and byte-exact source/image reversal;
- client authentication and authority checks at the real daemon transport.

Evidence should identify the exact source revision, toolchain, isolation mode,
case sets, resource ceilings, artifact digest, signature, canary journeys, and
restored known-good digest. Never convert an unavailable isolation or signing
step into a pass.

## Key source locations

- `crates/meta-harness/src/` — trace intake, diagnosis, candidates, independent
  evaluation, and promotion state machine
- `crates/self-evolution/src/enablement.rs` — installation availability and
  editable/protected surface projection
- `crates/self-evolution/src/guard.rs` — compiled protected paths, change
  classes, and consent policy
- `crates/self-evolution/src/shadow.rs` — commit-exact staging, manifests,
  quarantine, and safe reclamation
- `crates/self-evolution/src/build.rs` — isolated candidate build gates
- `crates/self-evolution/src/budget.rs` — evolution resource and recursion
  limits
- `crates/self-evolution/src/watchdog.rs` — canary observation and restoration
- `crates/self-evolution/src/ledger.rs` — durable audit history
- `crates/protocol/src/lib.rs` — evolution commands, availability, status, and
  projections
- `crates/daemon-core/` — authority enforcement and client integration
- `evidence/meta-harness/qualification.json` — focused deliberate-defect
  qualification evidence
- `spec/keith-everywhere/spec.kvx` — cross-surface implementation and gate
  status
