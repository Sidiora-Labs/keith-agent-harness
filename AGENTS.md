# Keith repository guide for coding agents

Keith is a local-first, open-source agent runtime. One durable Keith identity is
shared across the Web app, terminal UI, desktop app, APIs, channels, plugins,
and computer-use sessions. Its defining capability is self-evolution: Keith can
change its own harness, evaluate candidate versions outside the candidate's
control, promote improvements gradually, and return to a known-good version.

This is the canonical repository-wide guide for coding agents. Read it before
editing the project, along with the documentation and tests closest to the code
you are changing.

## Authority and scope

- Work only on the task you were given. Repository instructions constrain how
  that work is done; they do not grant permission for unrelated changes.
- Do not deploy, publish, push commits, rotate credentials, mutate external
  services, or perform destructive cleanup unless the request explicitly calls
  for it.
- Treat an existing dirty worktree as user work. Inspect it before editing,
  preserve unrelated changes, and never discard files to make the tree clean.
- If the request is only to inspect, explain, review, or gather context, stay
  read-only.
- Never claim a test, integration, platform, or release passed unless you ran
  the relevant proof and observed the result.

## Start here

Read the documents that match the work:

- [README.md](README.md) for the product and supported surfaces.
- [CONTRIBUTING.md](CONTRIBUTING.md) for contributor setup and pull-request
  expectations.
- [SECURITY.md](SECURITY.md) before changing authentication, authority,
  credentials, sandboxing, integrations, computer use, or self-evolution.
- [docs/crate-guide.md](docs/crate-guide.md) for crate ownership.
- [docs/architecture/dependency-boundaries.md](docs/architecture/dependency-boundaries.md)
  for allowed dependency direction.
- The applicable file under `spec/` when the request names a feature or spec
  task. Specs describe acceptance criteria; they do not expand the request.

If a more specific `AGENTS.md` exists below the directory you are editing, it
adds to or overrides this repository-wide guidance for that subtree.

## Architectural invariants

Preserve these properties across every implementation:

1. **One Keith, many clients.** The Web, TUI, desktop, API, channel, and ACP
   surfaces are views onto the same agent and durable state, not independent
   agent implementations.
2. **The daemon owns durable truth.** Clients render projections and send typed
   commands. They must not invent session, lifecycle, approval, or integration
   state locally.
3. **Dependencies point inward.** Versioned contracts sit below domains;
   adapters implement domain-owned traits; orchestration composes them; apps are
   composition roots. Library crates never depend on applications.
4. **Authority is explicit.** Profile, workspace, external principal, approval,
   credential, tool, plugin, and control-lease boundaries must survive every
   transport and adapter.
5. **Models and external content are untrusted.** Prompts, pages, files, channel
   messages, tool output, and retrieved memory never decide security policy.
   Deterministic code enforces authentication, authorization, approvals,
   sandboxing, evaluation, and promotion.
6. **Self-evolution stays independently governed.** A candidate harness cannot
   modify its evaluator, held-out tests, security policy, approval rules,
   runtime authority, promotion gate, or rollback mechanism.
7. **Failure is part of the design.** Cancellation, retry, reconnect,
   crash/restart recovery, idempotency, and honest degraded states are normal
   behavior, not optional cleanup.
8. **Profile data stays profile-scoped.** Sessions, credentials, memories,
   artifacts, integrations, external accounts, and delivery queues may not leak
   across profiles.

## Repository map

| Area | Responsibility |
| --- | --- |
| `apps/agentd` | Durable daemon and control plane |
| `apps/agent-worker` | Isolated worker process |
| `apps/agent-web` | Web server, native platform API, and OpenAI-compatible API |
| `apps/agent-web/ui` | React/TypeScript Web interface |
| `apps/agent-tui` | Terminal client |
| `apps/agent-desktop` | Desktop application |
| `apps/agent-acp` | Agent Client Protocol server |
| `apps/channel-gateway` | External channel ingress and delivery |
| `apps/cua-runner` | Computer-use runtime |
| `apps/xtask` | Repository checks, packaging, and release automation |
| `crates/agent-types`, `crates/protocol`, `crates/platform-contracts` | Shared versioned contracts |
| `crates/agent-loop`, `crates/session`, `crates/daemon-core`, `crates/worker-runtime` | Runtime and session orchestration |
| `crates/meta-harness`, `crates/self-evolution` | Candidate diagnosis, evaluation, promotion, and rollback |
| `crates/cua`, `crates/task-recipe` | Computer control, teaching, and replay |
| `crates/plugin-*`, `crates/composio` | Plugin and connected-app boundaries |
| `crates/channel-*`, `crates/delivery` | Channel contracts, adapters, and durable delivery |
| `crates/credentials`, `crates/profile`, `crates/data-control`, `crates/sandbox` | Security and data boundaries |
| `crates/ui-model` | Shared UI projections |
| `deploy`, `packaging`, `scripts` | Deployment, distribution, and operational tooling |
| `evidence`, `tests/security` | Qualification evidence and cross-surface security cases |

Put a change in the lowest layer that owns the behavior. Shared wire values
belong in a versioned contract crate; domain policy belongs in the domain;
provider-specific behavior belongs in an adapter; process assembly belongs in
an app. Do not bypass a boundary by reaching into another crate's storage or
copying its state machine into a client.

## Working in the repository

1. Inspect `git status`, the relevant docs, the owning module, its callers, and
   its tests before editing.
2. Use `rg` or `rg --files` to find existing types and patterns. Extend the
   established contract instead of creating a parallel abstraction.
3. Make the smallest complete change that solves the requested behavior.
4. Add or update tests at the boundary where the regression would be visible.
5. Run focused validation first, then broader gates in proportion to the
   change's risk.
6. Review the final diff for unrelated edits, generated artifacts, secrets, and
   accidental compatibility changes.
7. Report what changed, exactly what was run, and anything that remains
   unverified or blocked.

Files marked with a generated banner must be changed through their source or
generator. Do not hand-edit generated spec mirrors, API artifacts, lock-derived
metadata, or vendored output.

## Contributor commands

Use the repository wrapper for normal workflows. It pins the expected tools and
keeps Cargo output outside the checkout.

```bash
./keith doctor
./keith setup
./keith dev
./keith check
./keith test
./keith build
./keith image keith-agent:dev
```

`./keith help` lists scaffolding, Docker Compose, and cloud deployment commands.
Deployment commands are plan-only unless the operator supplies both explicit
execution flags and approval.

For a focused Rust check, use a disposable target directory and lint only the
package being changed:

```bash
(
  keith_target="$(mktemp -d /tmp/keith-check.XXXXXX)"
  trap 'find "$keith_target" -depth -delete' EXIT
  CARGO_TARGET_DIR="$keith_target" CARGO_INCREMENTAL=0 \
    cargo test -p keith-agent-tui --locked
  CARGO_TARGET_DIR="$keith_target" CARGO_INCREMENTAL=0 \
    cargo clippy -p keith-agent-tui --all-targets --no-deps --locked -- -D warnings
)
```

Replace `keith-agent-tui` with the package that owns the change. Never create or
commit a repository-local `target/` directory.

For the Web UI:

```bash
corepack pnpm --dir apps/agent-web/ui check
corepack pnpm --dir apps/agent-web/ui test
corepack pnpm --dir apps/agent-web/ui build
```

The repository currently targets Rust 1.93.0, edition 2024, and pnpm 11.18.0.
Rust formatting is mandatory, `unsafe` is forbidden in workspace crates, and
Clippy warnings are denied under the workspace policy.

## What proof is expected

- **Rust logic:** focused unit or integration tests plus strict Clippy for every
  changed package.
- **Protocol or API behavior:** success, malformed input, authentication,
  authorization, profile isolation, cancellation, and restart behavior as
  applicable. Exercise the real transport, not only a helper function.
- **Web or TUI behavior:** component/model tests plus the real interactive path
  in a browser or terminal. Confirm pending, streaming, completed, failed,
  reconnect, empty, and narrow-screen states when they are affected.
- **Persistence:** restart or migration coverage using the real store and
  serialized types.
- **Security boundaries:** adversarial tests at the actual ingress and the
  relevant checks from `SECURITY.md`.
- **Packaging or deployment:** build the release artifact or image and run its
  smoke journey. A successful library test is not packaged-runtime proof.
- **Release qualification:** use the broader `cargo ci`, `cargo
  clean-checkout`, `cargo platform-gate`, and `cargo security-gate` commands as
  applicable.

Do not use mocks, stubs, or fake internal collaborators to declare a real Keith
path qualified. A deterministic adapter at an unavoidable external boundary is
acceptable only when the production contract and real orchestration path remain
under test. When credentials or infrastructure are unavailable, record the
blocked journey instead of converting it into a pass.

## Security rules for changes

- Never write credentials, bearer tokens, signing keys, personal data, or
  unredacted traces into source, fixtures, logs, screenshots, or evidence.
- Keep credential values opaque and server-side. Pass references across
  boundaries, not secret material.
- Preserve separate authentication for the Web login, native platform API, and
  OpenAI-compatible API.
- Keep network listeners on loopback by default. Do not weaken TLS, origin,
  ingress, or proxy assumptions for convenience.
- Validate paths, symlinks, URLs, callback state, signatures, replay windows,
  resource bounds, and external account ownership at deterministic boundaries.
- Treat plugins, skills, MCP servers, connected apps, channel senders, and
  browser content as capability-bearing, untrusted inputs.
- Never relax an evaluator, benchmark, approval, authority, or promotion rule
  merely to make a self-evolution candidate pass.

Report suspected vulnerabilities privately using the process in
[SECURITY.md](SECURITY.md).

## Completion checklist

Before handing work back, confirm that:

- the requested behavior is complete and the diff contains only intended work;
- architecture, authority, profile, and compatibility boundaries are intact;
- tests exercise the real path and their exact results are reported;
- user-facing behavior and public contracts have matching documentation;
- no secrets, local state, build output, or repository-local `target/` were
  added; and
- any unrun gate, unavailable credential journey, or remaining risk is stated
  plainly.
