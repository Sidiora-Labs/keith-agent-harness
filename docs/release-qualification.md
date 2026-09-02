# Release qualification

This is the acceptance checklist for a Keith release candidate. Run it against
the exact source revision, signed release directory, and OCI digest that will be
published. Loose development binaries, focused crate tests, and a healthy HTTP
response are useful evidence, but none is a substitute for the complete
packaged journey.

Record the source commit, version, build ID, target platform, image digest,
release-manifest digest, provider and model, workspace and data-root strategy,
commands, exit status, elapsed time, and any blocked row. Never convert a
missing credential, platform, or external service into a pass.

## 1. Source and dependency gates

From a clean checkout with Cargo output outside the repository:

```bash
cargo ci
cargo clean-checkout
cargo dependency-policy
cargo deny check
cargo audit
cargo security-gate
```

Run `cargo platform-gate` on Linux, macOS, and Windows. Run the Web type-check,
tests, and production build with the locked Node and pnpm versions. Validate
shell scripts, GitHub Actions, Docker Compose, the Helm chart, and every
non-mutating cloud deployment plan.

The source gate must leave the checkout clean and must not create a repository-
local `target/` directory.

## 2. Build identity and signed contents

1. Build the release with a non-development `KEITH_BUILD_ID` and the protected
   Ed25519 release seed.
2. Obtain the expected public key through an independent authenticated channel.
3. Run `cargo xtask verify-release RELEASE EXPECTED_PUBLIC_KEY_HEX` from the
   audited source checkout.
4. Confirm `agentd --build-info` and `agent-worker --build-info` match the
   signed manifest's version, build ID, protocol, schema, target, and features.
5. Confirm the archive contains the documented binaries, Web assets, provider
   catalog, built-ins, schemas, SBOM, license report, provenance, manifest, and
   detached signature—nothing else.
6. Verify that unlisted files, duplicate paths, symlinks, unsafe paths, changed
   permissions, an altered payload byte, a wrong key, and a wrong build report
   are rejected on disposable copies.

Do not use a newly downloaded release's own verifier as the only trust root for
that same download.

## 3. Clean installation and first turn

Use a clean account or machine and new absolute `STATE_ROOT`, `DATA_ROOT`, and
`WORKSPACE_ROOT` paths:

1. Initialize desktop state and inspect the saved settings.
2. Configure one real provider through the environment-backed credential flow;
   confirm its plaintext value does not appear in process arguments, files,
   logs, diagnostics, or browser responses.
3. Start the packaged daemon with the packaged worker.
4. Attach the packaged TUI and complete a real streaming model turn.
5. Start the packaged Web app, authenticate, resume the same session, and
   complete a turn that uses an approved workspace tool.
6. Confirm the committed user message, activity, tool result, assistant answer,
   usage, and resulting file are visible after reconnect.
7. Exercise cancel, denied approval, accepted approval, provider failure, tool
   failure, and a context-compaction boundary.

The user message must become visibly pending after authoritative acceptance;
partial provider text must not be recorded as a successful final answer.

## 4. API and client boundaries

With separate credentials for each HTTP surface:

- authenticate and exercise OpenAI-compatible model discovery, one
  non-streaming Chat Completion, one streaming completion, usage reporting,
  durable conversation resume, and every documented unsupported-feature error;
- authenticate the native platform catalog and capabilities routes, create a
  session, stream a command to its terminal result, reconnect to the event
  stream with a cursor, and prove wrong-profile access fails;
- run the ACP stable-v1 lifecycle over stdio and each managed transport being
  shipped, including initialize, create, load, resume, fork, cancel, close,
  permission, attachment, terminal, and MCP capability behavior;
- verify that Web login, OpenAI API, and native API credentials cannot be used
  interchangeably.

Protocol qualification includes malformed, oversized, unauthorized, stale-
cursor, reconnect, backpressure, and daemon-unavailable cases.

## 5. Optional feature journeys

Qualify every feature advertised for the release. When a release does not ship
or enable a feature, record that fact in the manifest and release notes.

- **Computer use:** headed and headless launch, bounded observation/action,
  screen streaming, human takeover, exclusive lease expiry, crash recovery,
  credential redaction, and resource ceilings.
- **Teaching:** demonstration capture, sanitization, recipe editing, changed-
  layout replay, correction, version comparison, rollback, and approved skill
  publication.
- **Self-evolution:** deliberate harness defect, trace diagnosis, candidate
  population, private evaluation, signed build, canary, observation, automatic
  reversal, manual reversal, crash recovery, budgets, and protected-surface
  attacks.
- **Plugins:** real Wasmtime component invocation, capabilities, limits,
  provenance, install/update/rollback/uninstall, safe mode, hostile package,
  crash, and state migration.
- **MCP and connected apps:** schema bounds, profile isolation, credential
  references, callback verification, endpoint/SSRF policy, approval-gated
  mutation, cancellation, restart, and hostile results.
- **Personal intelligence:** memory evidence and correction, retrieval fallback,
  knowledge links, goals, children, plans, schedules, waits, commitments,
  awareness, initiative, and restart-safe delivery.

Local conformance is not external-service qualification. Record real-account
credentials and owner authorization as blockers when they are unavailable.

## 6. Channels and delivery

For every channel advertised as externally qualified, use a real authorized
account and exercise:

1. exact account identity and read-only connection testing;
2. verified inbound direct, mention, thread, and attachment behavior supported
   by that adapter;
3. outbound reply, artifact, scheduled return, and status behavior;
4. reconnect, cursor replay, deduplication, rate limiting, retry, and revocation;
5. denied conversations, wrong account/profile routes, forged signatures, stale
   delivery claims, and duplicate-risk projection after unknown outcomes.

Publish the exact supported and unsupported capability matrix for Discord,
Slack, Telegram, WhatsApp Cloud, Microsoft Teams, Google Chat, email, and
Matrix. An adapter implementation or loopback test alone is not a real-account
pass.

## 7. Restart, backup, migration, and rollback

1. Stop clients and terminate the daemon normally; confirm workers drain.
2. Restart the same release and confirm the selected session, committed answer,
   tool result, active goal/wait, workspace file, and client cursor recover.
3. Repeat with abrupt worker, daemon, and supervised child termination at each
   durable boundary; reconcile external effects before retry.
4. Create a backup, verify its manifest and digests, reject modified and
   symlinked copies, and restore into an empty data root.
5. Run the migration matrix from every supported predecessor schema.
6. Install a second release from the same trusted publisher, prove data and
   protocol compatibility, then roll back to the retained version.
7. Reject a release signed by another key or incompatible with the stored
   schema.

## 8. Container and deployment artifact

Build the production image from the release source and run:

```bash
scripts/ci/container-smoke.sh keith-agent:release 17341
```

Then verify the published multi-architecture digest on `linux/amd64` and
`linux/arm64`. Confirm the daemon, Web app, authenticated bootstrap, OpenAI
model discovery, persistent volume, non-root user, health check, signal
forwarding, workspace mount, restart, and secret import behavior.

Render and inspect Docker Compose, Helm, Railway, Fly.io, DigitalOcean, Azure,
AWS, and Google Cloud plans. A plan validation is not a deployed-service pass;
record live provider journeys separately when they are part of the release bar.

## 9. Performance and soak

Run the packaged performance runner and the declared multi-hour soak workload.
Record latency percentiles, time to first visible activity, completion latency,
throughput, CPU, resident memory, queue depth, tokens, reconnect time, and
resource-reclamation behavior under representative concurrency.

Compare against the checked release thresholds. A run that exceeds a required
budget fails qualification even when its functional assertions pass.

## 10. Uninstall and data choices

Exercise each uninstall plan on a disposable installation:

- `keep-user-data` removes installed versions while retaining documented user
  data;
- `remove-runtime` additionally removes transient runtime and notification
  state while retaining documented durable data;
- `remove-everything` removes the selected state and data roots after the exact
  confirmation phrase.

Inspect the native credential store separately and prove there are no
undocumented remnants in the selected parent locations.

## Sign-off

A release is qualified only when every required row has reproducible evidence
for the declared platform and artifact. The release report must list excluded
features, unqualified external integrations, unsupported platforms, waived
performance thresholds, and all remaining risks. If any required row is blocked
or fails, the report says so plainly and the release remains unqualified.
