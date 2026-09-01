## Product stance

Keith Self-Evolution lets Keith improve his own machinery and prove the improvement before it becomes real. It is deliberately narrow. Keith does not tune his own persona, memory, prompt weights, or routing knobs, because none of those changes can be falsified: nothing can tell the user whether the agent got better or merely drifted. Keith edits his own Rust source, because source is the one surface where "test yourself" is a literal statement. It compiles or it does not. The suite passes or it does not. The security gate holds or it does not.

The feature is off until the owner turns it on, and every change it ever makes is reversible in one action.

## The protected kernel

Two crates are permanently outside the editable surface: `crates/agent-loop` and `crates/memory`. The loop is the thing executing the evolution, so corrupting it mid-swap has no recovery path. Memory is identity and continuity, and a bad edit there loses the self rather than a feature. Everything else in the workspace is fair game.

A third protection is a logical necessity rather than a policy choice: the guard that enforces the protected set, the evolution ledger, and the release verifier are themselves immutable. A rule Keith can edit is not a rule. `ProtectedSurface` is compiled into the daemon from a constant, is checked against the shadow tree before any build, and is re-checked against the promoted artifact before any worker roll. Widening it requires a human editing source and rebuilding by hand.

## Enablement

Self-evolution is disabled by default in every profile and every packaged build. Enabling is an explicit, per-installation act that names what becomes possible, requires a working toolchain, and records who enabled it and when. A build without a usable Rust toolchain reports the feature as unavailable rather than silently accepting proposals it can never verify. Disabling is immediate: in-flight proposals abort, staged artifacts are discarded, promoted changes stay in place until reverted explicitly, and no new evidence is collected.

## The cycle

Evidence comes from what Keith already records: `ExperienceService` outcomes, telemetry counters, repeated failure categories, and crash reconciliation results. Evidence alone never justifies a change.

A proposal begins as an `EvolutionHypothesis`: a falsifiable statement with a named metric, a target crate, a measurement slice, and a revert threshold stated before any code is written. "Make planning better" is not a hypothesis. "Coding-category turns fail with `Verification` at 22 percent against model X; serializing the reviewer's second pass will reduce that below 12 percent on corpus slice C; revert if turn p95 latency rises more than 20 percent" is. A proposal with no metric is rejected at admission, not at review.

The reviewer receives a read-only bundle — selected source, the hypothesis, and the relevant failure evidence — and has no shell and no write tool, matching the existing refinement reviewer contract. It emits a bounded patch against a `ShadowTree`: a separate copy of the source tree that the live checkout never depends on. Keith never writes into the tree he is currently running from.

Verification runs in a sandboxed build with the same process limits as any other guest workload: `cargo fmt --check`, strict Clippy, the real workspace test suite, `xtask dependency-policy`, `xtask security-gate`, and `xtask platform-gate`. A `BuildVerdict` is only ever derived from real exit statuses. A model's claim that a change is safe carries no weight anywhere in this pipeline.

A passing build produces a candidate `WorkerImage`. The image is exercised by a canary worker — a real worker process started by `WorkerSupervisor` on a new generation, against a canary root tree, running the evaluation corpus and deterministic trace replay. This reuses lease and generation machinery rather than inventing a parallel runtime. The canary either satisfies the hypothesis metric or the transaction ends there.

## Hotswap

Worker binaries swap live. The supervisor's fixed `executable` becomes a resolved `WorkerImage` reference with a current pointer and a pinned previous-known-good pointer. Promotion installs the new image, then rolls each active root tree through the existing drain-and-replace path, so every root tree lands on a new generation running the new binary while the daemon never restarts and no session tree is lost. Rollback is the same operation with the pinned pointer.

Daemon changes cannot swap live, and the design does not pretend otherwise. A change touching daemon-resident code is staged: verified, signed, and marked for the next supervised restart. If the new daemon fails to open its endpoint, the previous image is restored automatically and the failure is surfaced.

## Reversal

Every promotion is one ledger entry with one reversal action. Reverting restores the pinned image, rolls workers back, and restores the source bytes in the live tree, as a single transaction with no further confirmation. Reversal does not require the reverted change to still build, because reversal restores recorded bytes and a recorded artifact rather than recomputing anything. A baseline restore reverts every self-evolution change back to the last human-approved state in one action.

Separately, an automatic `RevertWatchdog` observes declared SLOs for a bounded window after each promotion and reverts without asking if they regress. This is what makes autonomous promotion acceptable: the blast radius is bounded in time, not permanent.

## Ledger

Every hypothesis, proposal, verdict, canary result, promotion, revert, and post-swap observation becomes an append-only signed record. The ledger is the product surface: the owner can read what Keith changed about himself, why, what the evidence was, what the measurement showed, and can undo any of it. Records are never rewritten, and a reverted change keeps its history rather than disappearing.

## Anti-goals

Keith does not replicate himself, does not widen his own permission set, does not edit the protected surface or the guard, and does not spawn evolution work from within evolution work. Self-improvement consumes a declared budget of compute, tokens, wall time, and build slots, and is suppressed before interactive work is delayed. Public protocol and persistence-schema changes require explicit human approval regardless of class, because an applied migration is not something an auto-revert can undo.

## Qualification

Qualification runs against real processes. Builds are real cargo invocations. Canaries are real worker processes on real generations. Rollback is proven by killing processes at every durable boundary and confirming the live tree, the installed image, and the ledger agree afterwards. The adversarial suite proves that a hostile proposal cannot reach the protected surface, cannot escape the build sandbox, cannot exfiltrate credentials through a test, and cannot promote an artifact whose gate did not actually pass.
