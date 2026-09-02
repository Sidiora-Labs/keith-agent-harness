# Channel adapter development

This guide is for contributors adding a messaging provider to Keith's shared
channel system. A provider adapter translates an authenticated external event
into `ChannelEventV2` and a capability-checked `ChannelOperationV2` into one
external API action. It does not choose profiles, run the model, own the daemon,
or invent delivery success.

Read [Messaging channels](messaging-channels.md) first for operator behavior and
the current qualification status.

## Choose the owning layer

Keep responsibilities separated:

| Layer | Owns |
| --- | --- |
| `keith-channel-core` | Provider-neutral contracts, conformance, retry classes, queueing, reconnect, and `AgentConnection` |
| `keith-channel-adapters` | Provider authentication, normalization, API requests, cursor state, and exact capability declarations |
| `keith-channel-gateway` library | Account registration, routing, replay protection, worker isolation, fairness, health, and delivery partitions |
| `channel-gateway` executable | Process arguments and currently wired Discord/JSONL operation |
| `agentd` / protocol | Profiles, sessions, integration authority, durable delivery, and client projections |

Do not add provider-specific fields to the shared contract when they can remain
bounded adapter metadata. Change the contract only when multiple providers need
the same stable semantic.

## Implement the v2 contract

Add the provider module under `crates/channel-adapters/src/` and implement
`ChannelAdapterV2`:

```rust
pub trait ChannelAdapterV2 {
    fn capabilities_v2(&self) -> ChannelCapabilitiesV2;
    fn receive_v2(&mut self) -> Result<ChannelEventV2, ChannelAdapterErrorV2>;
    fn execute_v2(
        &mut self,
        operation: &ChannelOperationV2,
    ) -> Result<ChannelOperationReceiptV2, ChannelAdapterErrorV2>;
    fn reconnect_v2(&mut self) -> Result<(), ChannelAdapterErrorV2>;
    fn reconnect_cursor_v2(&self) -> Option<ReconnectCursorV2>;
}
```

Also implement `ManagedChannelAdapter`, normally through the existing macro in
`crates/channel-adapters/src/lib.rs`. The managed surface supplies:

- a stable `ChannelAdapterKind`;
- secret-free `ChannelAccountSetupV2` diagnostics; and
- a read-only `test_connection` that verifies the configured account identity.

Add the kind to `ChannelAdapterKind::ALL`, the built-in definition, public
exports, catalog registration, and qualification evidence. The built-in
definition is the source of truth for required credential names, scopes,
supported ingress modes, and safe connection testing.

## Declare capabilities truthfully

`ChannelCapabilitiesV2` must include a declaration for every capability:

- inbound/outbound messages;
- threads, replies, and mentions;
- commands, edits, deletion, and reactions;
- attachments, voice, and rich content;
- typing, delivery receipts, and read receipts; and
- rate limits, reconnect, cancellation, and idempotent send.

Every unsupported declaration needs a non-empty safe reason. Bounds for event
bytes, attachment bytes/count, rich-content bytes, and optional request rate
must be positive. Call `ChannelConformanceV2::admit_event` before returning an
event and require the operation's capability before touching the provider.

Do not label a send idempotent merely because Keith supplied an idempotency key.
The provider request must enforce that identity. If a response can be lost after
the provider applied the effect, return `UncertainAcknowledgement` and set the
receipt's duplicate risk accurately.

## Normalize inbound events

Map provider payloads into stable contract values:

- `account_id` is the configured external account, not a display label;
- conversation, thread, reply target, sender, event, and message IDs retain the
  provider's stable identities;
- bot-authored messages are identified and suppressed where required;
- timestamps are provider-derived when trustworthy and bounded;
- edits, deletions, reactions, commands, typing, receipts, rate limits,
  reconnect, and cancellation use their corresponding event variants;
- attachments include kind, media type, byte length, and only approved download
  or staging locations; and
- provider-only fields go into bounded metadata, never into authority fields.

Malformed or unauthenticated input must fail before model or tool invocation.
Provider error bodies, user names, subjects, and message text are unsafe data;
return a bounded `safe_message` instead of forwarding arbitrary remote content
as an operational error.

## Webhook, polling, and socket ingress

For webhooks:

1. bound headers and raw body;
2. identify the configured account from trusted routing state;
3. verify signature, audience, timestamp, and freshness over the original body;
4. admit the delivery ID through the durable account-scoped replay guard; then
5. parse and normalize the payload.

The Teams, Google Chat, and email modules deliberately receive a verifier trait
at their ingress boundary. A production composition must provide cryptographic
verification; a parser test is not a verified webhook test.

For polling or sockets, preserve the provider cursor needed to continue after a
restart. Reject a stale or blank cursor instead of skipping an unknown range.
Heartbeats, reconnect requests, provider rate-limit frames, and terminal
authentication failures must remain distinguishable.

## Outbound operations

Validate the exact route before sending:

- channel and external account match this adapter instance;
- conversation, thread, reply target, and message identity are valid;
- requested operation was declared supported;
- text, attachments, and rich content fit adapter bounds;
- artifact bytes match the staged length and digest; and
- any provider-specific idempotency or transaction ID is stable across retry.

Return `ChannelOperationReceiptV2` with the provider message identity when one
exists, the accepted timestamp, terminal or uncertain state, duplicate
possibility, and safe metadata. A successful HTTP write with a malformed receipt
is not a confirmed delivery.

## Account lifecycle and routing

Register real adapter instances through `ChannelAdapterCatalog`; do not create a
second provider registry. `ChannelGatewaySupervisor` owns one record and queue
per `(kind, account_id)` and the runtime owns one worker per record.

Inbound routing rules can require a direct conversation or explicit mention and
must resolve to an exact profile/session. Outbound delivery is claimed by
`(channel, external_account)`. Preserve this partition when adding background
or scheduled delivery so one account cannot drain another account's work.

Credential rotation increments the account's generation and replaces only that
worker. Pause, resume, reconnect, test, remove, and rate-limit transitions must
remain local to the affected account. Health projections may include stable
identities, counters, cursor presence, timestamps, lifecycle, and a safe error;
they must never include credentials or raw provider responses.

## Testing requirements

Add coverage in both the adapter crate and gateway where applicable:

1. valid setup, exact credential names/scopes, and all 19 capability declarations;
2. official signature vectors or a real verifier implementation, including
   tampering, wrong account/audience, stale timestamp, malformed payload, and
   verification-before-parse;
3. messages, threads/replies, every claimed event type, bot suppression,
   duplicates, and cursor resume;
4. outbound success, malformed receipt, rate limit, retry, uncertain effect,
   wrong route/account, and every claimed operation;
5. attachments/media, byte limits, digest mismatch, unsafe URLs, and cleanup;
6. account fairness, queue bounds, pause/resume, credential rotation, worker
   crash, reconnect, replay, removal, and delivery partition isolation; and
7. daemon and client projection behavior for any newly exposed lifecycle.

Use controllable loopback servers to exercise real HTTP/WebSocket encoding and
transport behavior without claiming an external account. Do not replace the
production adapter with a fake collaborator to obtain a green result.

Focused checks:

```bash
cargo test -p keith-channel-core -p keith-channel-adapters -p keith-channel-gateway --locked
cargo test -p keith-channel-adapters --locked -- --ignored
cargo clippy -p keith-channel-core -p keith-channel-adapters -p keith-channel-gateway \
  --all-targets --no-deps --locked -- -D warnings
```

Use an external disposable Cargo target. If the adapter is intended for the
standalone executable, add and prove its process arguments and ingress server;
a registered library adapter alone does not establish runnable setup.

## Qualification and documentation

Update `evidence/channels/qualification.json` with:

- the exact platform and implementation path;
- required credential references;
- explicitly unsupported capabilities;
- local commands and observed counts;
- external account inputs and owner actions still required; and
- the real-account result.

A live qualification needs authorized external credentials and must exercise
inbound, outbound, restart, rate limit, duplicate, attachment, scheduled return,
revocation, and denial paths against the real provider. Keep it blocked if any
required journey was not run.

Update the operator guide only when a complete composition path exists. Include
credential-reference creation, webhook/socket/polling setup, provider scopes,
process invocation, lifecycle controls, recovery, removal, and redacted
troubleshooting.

## Review checklist

- [ ] Provider-specific behavior stays in the adapter.
- [ ] Required credentials and scopes are explicit references.
- [ ] Capabilities and limits are complete and truthful.
- [ ] Authentication precedes parsing and durable replay admission.
- [ ] Profile/session routing comes from trusted configuration.
- [ ] Outbound routes are exact-account checked.
- [ ] Retry and uncertain acknowledgement preserve side-effect truth.
- [ ] Cursor, replay, queue, worker, and delivery state survive restart.
- [ ] Errors, projections, fixtures, and evidence contain no secrets.
- [ ] Local conformance and real-account qualification are reported separately.

## Key source locations

- `crates/channel-core/src/lib.rs` — contract and conformance API
- `crates/channel-adapters/src/lib.rs` — built-in catalog and managed adapter glue
- `crates/channel-adapters/tests/catalog.rs` — all-adapter registration contract
- `crates/channel-adapters/tests/qualification_matrix.rs` — evidence consistency
- `apps/channel-gateway/src/lib.rs` — account supervision and worker runtime
- `apps/channel-gateway/tests/supervisor.rs` — multi-account lifecycle proof
- `apps/channel-gateway/src/main.rs` — currently exposed process modes
- `crates/protocol/src/lib.rs` — channel account commands and projections
- `crates/daemon-core/src/integrations.rs` — integration authority and profile policy
- `evidence/channels/qualification.json` — qualification ledger
