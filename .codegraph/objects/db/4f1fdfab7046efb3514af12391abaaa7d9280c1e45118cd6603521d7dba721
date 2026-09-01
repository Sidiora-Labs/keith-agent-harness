# Crate dependency boundaries

Keith Agent uses a strictly upward dependency graph:

1. `keith-agent-types` owns small versioned primitives and may use only serialization and utility dependencies.
2. Contract crates such as protocol, framing, provider-core, tool-core, channel-core, and repository traits depend on common types.
3. Domain crates own state machines and policies. They depend on contracts, never concrete providers, channels, storage engines, UI crates, or applications.
4. Adapter and storage crates implement domain-owned traits without reaching into session orchestration.
5. Worker-runtime, supervisor, daemon-core, and connection compose domain services.
6. Applications are composition roots and may depend downward. Library crates never depend on applications.

`cargo dependency-policy` reads Cargo metadata and rejects prohibited internal edges, including transitive upward dependencies. The policy is intentionally stricter for `session`, provider adapters, channel adapters, storage backends, and `agent-types`. New exceptions require an architectural spec change; the checker has no allow-by-comment escape hatch.

Crate ownership follows the directory boundary: common contracts live in `crates/agent-types`, protocol code in `crates/protocol` and `crates/framing`, domain behavior in its named domain crate, external integration in an adapter crate, orchestration in daemon/worker/supervisor crates, and executable assembly in `apps`. Cross-boundary public values belong in the lowest applicable versioned contract crate.

