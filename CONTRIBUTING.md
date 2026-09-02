# Contributing to Keith

Thank you for taking the time to contribute. Keith is developed in public, and
contributions of every size are welcome: bug reports, documentation, design
feedback, tests, integrations, performance work, and focused code changes.

This guide is the shortest path from a fresh checkout to a reviewable pull
request. For product context, start with the [README](README.md). For help with
an installation or configuration, use [SUPPORT.md](SUPPORT.md).

## Quick links

- [Issues](https://github.com/Sidiora-Labs/keith-agent/issues)
- [Discussions](https://github.com/Sidiora-Labs/keith-agent/discussions)
- [Security policy](SECURITY.md)
- [Architecture and crate guide](docs/crate-guide.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)

## Code of Conduct

By participating in Keith, you agree to follow the
[Code of Conduct](CODE_OF_CONDUCT.md). Be respectful, constructive, and patient
with people of different backgrounds and experience levels.

## How to contribute

Choose the path that matches what you have:

| Situation | Start here | Include |
| --- | --- | --- |
| Setup question or troubleshooting | [Support](SUPPORT.md) or [Discussions](https://github.com/Sidiora-Labs/keith-agent/discussions) | Version, OS, install method, safe logs |
| Reproducible bug or regression | [Bug report](https://github.com/Sidiora-Labs/keith-agent/issues/new/choose) | Reproduction, expected and observed behavior, impact |
| Feature or architecture proposal | [Discussions](https://github.com/Sidiora-Labs/keith-agent/discussions) before substantial work | User problem, desired outcome, alternatives, authority implications |
| Small documentation fix | Open a focused pull request | Affected page and verified replacement |
| Security vulnerability | [Private security advisory](https://github.com/Sidiora-Labs/keith-agent/security/advisories/new) | Follow [SECURITY.md](SECURITY.md); never disclose it publicly |

Search existing Issues and Discussions before creating a new thread. For a large
feature, protocol change, storage migration, new dependency, or authority-model
change, agree on the problem and design before investing in implementation.

## Development setup

### Prerequisites

| Tool | Version | Purpose |
| --- | --- | --- |
| Rust | 1.93 | Core runtime and applications |
| Node.js | 22.22 | Web UI toolchain |
| Corepack and pnpm | pnpm 11.18 | Locked frontend dependencies |
| Git | Current stable | Source and contribution workflow |

Docker, Helm, `kubectl`, Railway, Fly.io, and cloud-provider CLIs are optional
and only required for their corresponding build or deployment path.

From the repository root:

```bash
./keith doctor
./keith setup
./keith dev
```

`./keith doctor` checks required tools and reports optional ones. `./keith setup`
installs locked web dependencies and fetches locked Rust dependencies.
`./keith dev` builds and starts the daemon and Web UI at
<http://127.0.0.1:7341>.

The contributor command keeps Cargo build output outside the repository. Do not
create or commit a repository-local `target/` directory, `.env`, provider
credentials, session data, generated caches, or editor state.

### Environment

For Docker or deployment work, begin with the documented template:

```bash
cp .env.example .env
```

Use synthetic or disposable values for tests. Never commit `.env`, API keys,
OAuth tokens, Web login secrets, signing keys, credential databases, private
prompts, or unredacted traces.

## Project layout

```text
apps/                 executable composition roots and clients
crates/               contracts, domains, adapters, storage, and runtime layers
apps/agent-web/ui/    Web application
deploy/               Kubernetes, Railway, Fly.io, and cloud-provider tooling
docs/                 operator, integration, architecture, and release guides
evidence/             checked qualification evidence
packaging/            built-ins, provider catalog, and container runtime files
scripts/              CI and deployment helpers
spec/                 product requirements, tasks, and acceptance criteria
```

The dependency direction is intentional: contracts sit below domains, adapters
implement domain-owned traits, orchestration composes them, and applications are
the outermost layer. Read [dependency boundaries](docs/architecture/dependency-boundaries.md)
before moving types or adding cross-crate dependencies.

## Making changes

1. Start from the current default branch and create a short, descriptive branch.
2. Keep the change focused on one user or operational problem.
3. Read the nearby types, tests, documentation, and spec before editing.
4. Preserve existing protocol, authority, profile, credential, storage, and
   migration boundaries unless the accepted change explicitly revises them.
5. Add or update tests for behavior changes.
6. Update user-facing documentation when commands, configuration, APIs, or
   observable behavior change.
7. Run the smallest relevant checks while iterating, then the appropriate gate
   before requesting review.

Avoid unrelated refactors, dependency upgrades, generated-file churn, and broad
formatting changes. Do not weaken an assertion, permission check, approval gate,
or failure mode merely to make a test pass.

### User-interface changes

For Web or TUI changes, test the real interactive path and include before and
after screenshots or a short recording when the result is visual. Exercise
connection, sending, streaming, cancellation, resize, reconnect, and error states
when the changed surface can encounter them.

### Protocol and persistence changes

Protocol, schema, profile-ownership, session, credential, or durable-store
changes require compatibility and restart coverage. Document migration and
rollback behavior; do not silently reinterpret existing state.

### Security-sensitive changes

Changes to authentication, approvals, credentials, sandboxing, tools, plugins,
channels, APIs, computer use, self-improvement, release verification, or cloud
exposure must state the affected trust boundary and run the relevant security
and live-runtime checks. Read [SECURITY.md](SECURITY.md) first.

## Tests and checks

| Goal | Command |
| --- | --- |
| Inspect the local toolchain | `./keith doctor` |
| Formatting, Rust checks, and TypeScript checks | `./keith check` |
| Rust and Web test suites | `./keith test` |
| Full repository gate | `cargo ci` |
| Clean-checkout reproducibility | `cargo clean-checkout` |
| Security and authority gate | `cargo security-gate` |
| Production OCI image | `./keith image keith-agent:dev` |
| Live packaged container journey | `scripts/ci/container-smoke.sh keith-agent:dev 17341` |

The pull-request pipeline additionally covers Linux, macOS, and Windows
contracts; dependency advisories; deployment manifests; the OCI build; browser
authentication; daemon connectivity; and OpenAI-compatible model discovery.

Report the exact commands you ran. If a check is unavailable or blocked, include
the command, the relevant error, and what remains unverified. A focused passing
test is evidence for that surface, not proof that every platform or deployment
path is qualified.

## Test integrity

Tests exercise real Keith domain types, repositories, codecs, state machines,
process boundaries, and packaged binaries. Do not replace internal collaborators
with mocks, stubs, fakes, or placeholders merely to produce a passing test.

Deterministic providers and fault injectors are acceptable only as explicit
external-boundary adapters. They must implement the same production-facing
contract and exercise the real orchestration path, including serialization,
validation, cancellation, timeouts, and error classification.

Acceptance work that requires a real provider, process, browser, channel,
credential, signed package, or deployed service remains incomplete until that
path has actually passed.

## AI-assisted contributions

Contributions created with Codex, Claude, or another coding agent are welcome.
The author remains responsible for understanding the change, reviewing every
generated file, protecting secrets, keeping scope focused, and providing real
validation. AI-generated explanations or green checks are not substitutes for
source review and reproducible evidence.

## Pull requests

A good pull request makes the change easy to understand and verify:

1. Explain the user or operational problem.
2. Describe what changed and why this approach was chosen.
3. State user-visible impact and compatibility implications.
4. Link the Issue or Discussion when one exists.
5. List exact validation commands and results.
6. Include visual evidence for Web or TUI changes.
7. Call out security, authority, credential, storage, API, deployment, and
   rollback implications—or explain why none apply.
8. Keep maintainer edits enabled when opening from a fork.

Maintainers may ask for a smaller scope, more evidence, documentation, tests, or
design discussion before merging. Keep the pull-request description updated so
it remains the durable explanation of the work.

## Commit and pull-request titles

Use a short, imperative summary. Conventional Commit prefixes are encouraged but
not required:

```text
feat: add Matrix account supervision
fix: show submitted TUI messages immediately
docs: clarify Kubernetes secret setup
```

`CHANGELOG.md` is generated from Codify snapshots and repository history. Do not
hand-edit it as part of an ordinary pull request unless the release process
explicitly requires a correction.

Thank you for helping make Keith more reliable, useful, and accessible.
