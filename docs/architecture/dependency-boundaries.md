# Crate dependency boundaries

Keith is split into small Rust crates so security policy, durable state, external
adapters, orchestration, and user interfaces do not collapse into one process-
wide authority. The dependency graph is intentionally stricter than Cargo
requires: code may depend inward toward contracts and domain policy, never
upward toward an application or a concrete integration.

Run `cargo dependency-policy` after adding or moving an internal dependency.
The check reads Cargo metadata and rejects prohibited direct and transitive
edges. There is no allow-by-comment escape hatch; a genuine exception requires
an explicit architecture change.

## Layer model

```text
applications and process entry points
                 |
runtime composition and orchestration
                 |
adapters, repositories, and host implementations
                 |
domain state machines and policy
                 |
versioned contracts and common primitives
```

An outer layer may assemble lower layers. A lower layer must not know which app,
provider, channel, database, or UI happens to use it.

## Layers and ownership

| Layer | Representative packages | Owns |
| --- | --- | --- |
| Common primitives | `keith-agent-types` | IDs, timestamps, schema and protocol versions, structured errors |
| Versioned contracts | `keith-protocol`, `keith-framing`, `keith-provider-core`, `keith-tool-core`, `keith-channel-core`, `keith-runtime-api`, `keith-platform-contracts`, `keith-plugin-sdk` | Wire values, traits, capabilities, limits, and compatibility rules shared across a boundary |
| Domains | `keith-session`, `keith-goals`, `keith-memory`, `keith-cua`, `keith-meta-harness`, `keith-task-recipe` | State machines, invariants, admission rules, and policy |
| Adapters and storage | `keith-provider-adapters`, `keith-channel-adapters`, `keith-state-store`, `keith-composio`, `keith-plugin-host` | Concrete external protocols, persistence engines, and host implementations of domain-owned traits |
| Orchestration | `keith-local-runtime`, `keith-daemon-core`, `keith-supervisor`, `keith-worker-runtime`, `keith-connection` | Service composition, process leases, routing, recovery, and lifecycle |
| Applications | packages under `apps/` | Argument parsing, process assembly, listeners, UI hosts, and executable lifecycle |

The [crate guide](../crate-guide.md) lists every workspace package and its
current responsibility.

## Placement rules

### Put shared values in the lowest applicable contract

A value that crosses a process, transport, plugin, channel, or client boundary
belongs in the contract crate that owns that boundary. It must be versioned,
bounded, serializable, and validated independently of the caller.

Do not define slightly different copies of a profile ID, approval decision,
event cursor, or lifecycle state in multiple apps. Do not move application-
specific configuration into a common crate merely to make an import convenient.

### Domains own policy and traits

The domain that needs an external capability defines the trait and the policy
for using it. A provider, channel, repository, browser, or credential adapter
implements that trait from outside the domain.

This keeps a state machine testable without giving it ambient access to HTTP,
the filesystem, process spawning, credentials, or a concrete database. It also
prevents an adapter from deciding approval, profile ownership, or durable state
transitions for itself.

### Adapters translate; they do not become authorities

Adapters authenticate and normalize external data, implement retries and
transport-specific behavior, and return typed results. They may narrow granted
authority but never widen it. External payloads remain untrusted after parsing.

Examples:

- a channel adapter verifies the platform request before decoding it, then
  produces a normalized inbound message;
- a provider adapter converts Keith's provider-neutral request into one vendor
  protocol and maps the response back into typed events;
- the SQLite store enforces transaction and revision contracts but does not
  choose which profile may read a record;
- the plugin host enforces declared capabilities but does not invent grants for
  an installed package.

### Orchestration composes without duplicating domains

`local-runtime`, `daemon-core`, the supervisor, and worker runtime connect
domain services and move typed commands between them. They should not re-create
the session, goal, approval, memory, or promotion state machine in glue code.

Cross-service recovery belongs at the orchestration boundary when it must
reconcile several independently durable components. The individual domain still
owns the legal transitions for its records.

### Applications are composition roots

Executables may depend on the libraries they assemble. Library crates must not
depend on anything under `apps/`, import an app's argument type, or call an app
entry point. Shared behavior discovered in an app should move to the lowest
library that can own it without acquiring broader authority.

## Security-sensitive boundaries

Some dependency rules also enforce trust boundaries:

- provider and channel cores must not depend on concrete adapters;
- session code must not reach directly into credentials or storage backends;
- client projection code must not import daemon internals to manufacture state;
- self-evolution candidates must not gain dependencies on evaluators,
  installation authority, signing keys, promotion gates, or rollback control;
- plugins, MCP servers, browser content, and connected apps interact through
  bounded contracts rather than direct runtime access;
- profile-scoped domains must not open another profile's filesystem paths or
  repositories through a convenience dependency.

If a proposed dependency would make a lower layer aware of a credential value,
concrete host path, UI framework, transport session, or application lifecycle,
the responsibility is probably in the wrong layer.

## Adding a dependency

Before editing `Cargo.toml`:

1. Identify which layer owns the behavior and which crate owns the contract.
2. Search for an existing trait or value before adding a parallel abstraction.
3. Prefer a domain-owned trait plus an outer implementation over importing a
   concrete adapter.
4. Disable unnecessary third-party default features and document any new host,
   network, filesystem, process, or serialization surface in the pull request.
5. Add tests at the boundary whose invariant could now be violated.

Then run a focused package check and the graph policy:

```bash
cargo dependency-policy
cargo test -p PACKAGE --locked
cargo clippy -p PACKAGE --all-targets --no-deps --locked -- -D warnings
```

Use an external or disposable `CARGO_TARGET_DIR` as described in
[CONTRIBUTING.md](../../CONTRIBUTING.md). The full CI gate repeats the dependency
policy against the complete workspace.
