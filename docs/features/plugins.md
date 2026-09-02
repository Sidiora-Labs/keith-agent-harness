# Plugins

Keith plugins extend the runtime with typed tools, commands, and lifecycle
hooks packaged as WebAssembly components. The host validates the package and
its declared authority, runs it with deterministic memory, fuel, time,
concurrency, input, output, and host-call bounds, and keeps failures inside the
plugin boundary.

Plugins are executable code from another trust domain. Installation and
activation are separate from enabling a plugin for a profile, and neither step
grants ambient filesystem, network, process, environment, or credential access.

## Current status

The repository contains two host layers:

- `PluginHost`, used by `local-runtime`, provides bounded Wasm lifecycle,
  durable versions, activate/health/invoke/disable/rollback/uninstall, basic
  quarantine, and process-wide safe mode.
- `PluginAuthorityHost` is the qualified v2 package and authority lifecycle. It
  adds trusted publishers, SHA-256 package identity, Ed25519 verification,
  exact grant approvals, typed component calls, update diffs, migration,
  publisher continuity, call history, crash-loop safe mode, quarantine, and
  verified rollback.

The checked-in plugin qualification reports `passed_local_conformance` and a
real WASI component. It recorded 22 passing SDK/host tests plus strict Clippy at
the time of qualification. It is local component and lifecycle proof, not proof
of a public plugin marketplace or third-party package.

The full authority host is not yet mounted by the assembled local runtime. The
Web app can project generic plugin lifecycle records, but installation controls
are deliberately unavailable without a trusted approval path. There is no
public plugin install/sign/publisher-management CLI yet.

## Scaffold a component

Create a Rust component project:

```bash
./keith scaffold plugin calendar-tools
```

The default destination is `plugins/calendar-tools`; a second argument chooses
another directory. Names must match `[a-z][a-z0-9-]{1,62}`. The scaffold copies
the current WIT contract and reference Rust guest, which implements one typed
streaming `echo` tool.

Build it with `cargo-component`:

```bash
cargo component build --release \
  --manifest-path plugins/calendar-tools/Cargo.toml
```

This command produces a component binary, but the current scaffold is not a
complete installable Keith package. It does not generate `plugin.toml`, compute
and insert the module digest, sign the canonical manifest, register a trusted
publisher, or invoke an install API. Those are current tooling gaps; do not copy
the component into the data root and describe it as an authority-verified
installation.

## Guest component contract

The WIT package is `keith:plugin@1.0.0`. A component exports:

- `describe-tools` — typed callable descriptors;
- `describe-commands` — typed command descriptors; and
- `invoke` — one versioned invocation and one terminal response.

A descriptor includes a stable name and description, JSON input/output schemas,
risk, timeout, cancellation support, streaming support, concurrency limit, and
the exact grants it needs. The host validates both payloads against the schemas.
Stream frames have increasing sequence numbers and share the invocation's output
bound.

Lifecycle operations are activate, health, migrate, and deactivate. Callable
operations are tool and command and require an exact target. Responses terminate
as completed, cancelled, denied, or failed and may contain bounded payload,
stream frames, and a safe error.

The guest may request only typed host calls:

- HTTP to one declared host;
- one declared named credential;
- read or write in one declared plugin storage namespace;
- event emission;
- artifact creation;
- clock access; or
- safe logging.

Every host call can return an explicit denial. A plugin must handle denial and
cancellation as normal outcomes.

## Manifest and package

An installable package directory contains exactly the expected ordinary files:

```text
plugin.toml
plugin.wasm
```

Manifest v2 contains:

- package ID, name, version, host API range, WASI component kind, and hooks;
- resource ceilings and explicit grants;
- publisher ID/name/key ID;
- SHA-256 digest of `plugin.wasm`;
- Ed25519 signature and matching key ID;
- tool and command descriptors; and
- migration schema version and accepted prior plugin versions.

IDs are bounded lowercase tokens; versions and scopes are bounded. Wildcard
roots, hosts, environment, credentials, or storage namespaces are rejected.
Manifest v2 also rejects legacy ambient readable/writable roots, arbitrary
environment injection, and process access. Separate-process packages are
represented by the SDK enum but are not accepted by manifest validation.

The authority host loads ordinary non-symlink files under strict size limits,
verifies the module digest in constant time, verifies the canonical unsigned
manifest with a configured trusted publisher key, compiles the component, and
checks its exported descriptors against the manifest before installation.

## Installation, profile enablement, and invocation

The complete authority lifecycle is:

```text
package directory
  -> path, size, manifest, digest, signature, publisher verification
  -> component compile and descriptor validation
  -> exact approval for initial or widened grants
  -> immutable version storage
  -> optional migration in candidate version
  -> activation
  -> profile enabled_plugins intersection
  -> typed tool/command invocation
  -> bounded host calls, response validation, and call record
```

An update must retain publisher and signing-key identity. The host calculates
added/removed grants and tools and whether migration is required. New or widened
grants require a `GrantApproval` naming the exact plugin, source version, target
version, grant set, and human confirmation. A failed migration or activation
does not replace the selected good version.

In the current local runtime, active plugin IDs that also appear in the
profile's `enabled_plugins` list become tools named `plugin_<plugin-id>`. The
runtime integration presently invokes the plugin's general tool hook; it does
not yet expose each v2 descriptor as a distinct end-user tool or pass the full
authority-host lifecycle through the Web setup surface.

## Security boundaries

- Wasm components inherit no WASI imports or ambient host capabilities. An
  undeclared WASI import is rejected.
- Network hosts, credential names, storage namespaces, event/artifact access,
  clock, logging, fuel, memory, bytes, wall time, and concurrency are all
  explicit grants or limits.
- Named credential bytes are supplied only through the host call and are
  checked across payload, stream, and error output. Safe logs redact every
  observed secret.
- Input is schema-validated before execution; response identity, format,
  sequence, schema, and output bounds are validated after execution.
- Publisher trust and grant approval are host configuration. A manifest cannot
  declare itself trusted or approve its own widened authority.
- The profile enabled list can narrow which active packages become tools. A
  child session can narrow tools again but cannot widen plugin authority.
- Plugin output and emitted events remain untrusted content. They cannot alter
  approval, profile, evaluator, or runtime security policy.

## Failure, quarantine, safe mode, and rollback

- Fuel, memory, timeout, concurrency, output, host-call, schema, trap, and crash
  failures terminate the invocation without destabilizing later calls.
- Failed health checks quarantine the affected version. Repeated failures reach
  the crash-loop threshold and engage safe mode.
- Corrupt durable state, corrupt/incompatible packages, failed migration, an
  interrupted lifecycle, or an explicit operator request can engage safe mode.
- Safe mode prevents activation and invocation but keeps inspection, disable,
  rollback validation, and uninstall recovery available. Installed packages are
  represented as disabled.
- Exiting safe mode revalidates every selected package against its signed record
  and component contract. Plugins remain disabled until individually activated.
- Rollback reloads and revalidates an installed version, checks publisher
  continuity and any added grants, runs activation where allowed, and preserves
  the current version if rollback activation fails.
- Uninstall disables the plugin, removes all stored package versions and plugin
  state, and then removes its authority record.

## Current limitations

- The scaffold produces component source, not a signed installable package.
- Trusted-publisher and signing workflows currently exist as Rust APIs and test
  fixtures, not supported contributor CLI commands.
- `local-runtime` mounts the simpler `PluginHost`, not the qualified
  `PluginAuthorityHost`. Do not imply that every runtime-loaded package passed
  v2 signature and grant-approval enforcement until that integration changes.
- Web install and destructive lifecycle controls do not fabricate approval and
  are not a complete package-management UI.
- There is no public package registry or compatibility promise for third-party
  distribution beyond the versioned SDK/WIT contracts in this repository.

## Validate changes

Build the generated component, then run focused SDK and host proof:

```bash
cargo component build --release \
  --manifest-path plugins/calendar-tools/Cargo.toml
cargo test -p keith-plugin-sdk -p keith-plugin-host --locked
cargo clippy -p keith-plugin-sdk -p keith-plugin-host \
  --all-targets --all-features --no-deps --locked -- -D warnings
```

Use an external disposable Cargo target. Tests must use an actual component
binary for typed payload, stream, cancellation, grant denial, schema rejection,
credential redaction, ambient-import refusal, exhaustion, and crash isolation.
Lifecycle proof must cover signature/digest/traversal refusal, initial and
widened grant approval, update, migration failure, rollback, quarantine, safe
mode, restart, and uninstall.

When changing runtime integration, additionally prove profile enablement, exact
descriptor input/output, daemon/client lifecycle projection, approval flow, and
restart with the assembled runtime. `evidence/plugins/qualification.json` is the
current local qualification ledger.

## Key source locations

- `crates/plugin-sdk/src/lib.rs` — manifest, descriptors, grants, typed wire API,
  compatibility, and first-party extension registry
- `crates/plugin-sdk/wit/plugin.wit` — component-model contract
- `crates/plugin-sdk/reference-component/` — scaffold source
- `crates/plugin-host/src/abi.rs` — Wasmtime component execution and host calls
- `crates/plugin-host/src/authority.rs` — signed authority lifecycle
- `crates/plugin-host/src/lib.rs` — simpler host currently mounted by runtime
- `crates/plugin-host/tests/abi.rs` — real-component contract tests
- `crates/plugin-host/tests/authority.rs` — provenance, approvals, lifecycle,
  hostile package, safe-mode, and rollback tests
- `crates/local-runtime/src/lib.rs` — current runtime loading and tool exposure
- `apps/agent-web/ui/tests/apps.test.tsx` — secret-free lifecycle projection tests
- `evidence/plugins/qualification.json` — checked-in qualification result
