# Messaging channels

Keith's channel layer carries messages between one durable Keith profile and
external messaging accounts. Adapters translate provider-specific payloads into
the shared channel contract; the gateway assigns the configured profile and
session, submits the prompt to `agentd`, and returns completed work through the
durable delivery outbox.

Eight adapters are implemented: Discord, Slack, Telegram, WhatsApp Cloud,
Microsoft Teams, Google Chat, email, and Matrix. That is an implementation
statement, not a claim that eight live accounts have been qualified.

## Current status

The checked-in qualification result is
`local_conformance_passed_external_accounts_blocked`. Local contract, adapter,
loopback, supervision, and Web projection tests passed when the evidence was
recorded. No real external account is marked qualified because the required
credentials and owner authorization were unavailable.

| Adapter | Ingress implemented | Required credential references | Live-account status |
| --- | --- | --- | --- |
| Discord | Gateway/WebSocket | `bot_token` | Blocked |
| Slack | Events webhook or Socket Mode | `bot_token`, `signing_secret` | Blocked |
| Telegram | Webhook or polling | `bot_token`, `webhook_secret` | Blocked |
| WhatsApp Cloud | Signed webhook | `access_token`, `app_secret`, `verify_token` | Blocked |
| Microsoft Teams | Verified Bot Framework activity | `bot_framework_access_token`, `bot_framework_request_verifier` | Blocked |
| Google Chat | Verified event endpoint | `chat_api_access_token`, `google_chat_request_verifier` | Blocked |
| Email | Verified provider webhook or provider-specific polling | `provider_access_token`, `webhook_signature_verifier` | Blocked |
| Matrix | Client sync polling | `access_token` | Blocked |

Each adapter publishes an exact capability declaration. Unsupported operations
fail as unsupported; they are not emulated or silently dropped. See
`evidence/channels/qualification.json` for each adapter's current unsupported
capabilities and external blockers.

## What is implemented

The v2 channel contract covers:

- inbound and outbound messages, threads, replies, mentions, commands, edits,
  deletion, reactions, attachments, voice, rich content, typing, receipts,
  rate limits, reconnect, cancellation, and idempotent send;
- explicit supported or unsupported declarations for all 19 capabilities;
- bounded event, attachment, rich-content, and request-rate metadata;
- normalized identities, conversations, messages, operations, and receipts;
- classified authentication, permission, malformed-input, rate-limit,
  transient-network, permanent-destination, uncertain-acknowledgement,
  stale-cursor, cancellation, and unsupported-feature failures; and
- signature-before-parse webhook admission plus bounded reconnect cursors.

An adapter supports only the subset it declares. For example, the WhatsApp
adapter handles signed webhooks, outbound messages and templates, media,
delivery/read status, and mark-read operations, but it does not claim portable
message edits, reactions, rich content, or typing.

## Run the standalone gateway

The `channel-gateway` executable currently has two operating modes:

1. a complete Discord mode; and
2. normalized `RoutedInbound` JSON lines on standard input.

The other seven adapters exist as library and supervised-runtime components,
but this binary does not expose provider-specific command-line setup for them.
Do not present a library adapter or daemon lifecycle projection as a runnable
standalone gateway.

### Discord

Create and install a Discord bot with only the intents and permissions needed
for its authorized conversations. Pass the token through a named environment
variable, never as an argument:

```bash
export DISCORD_BOT_TOKEN='replace-with-the-secret'

bin/channel-gateway \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --discord-token-env DISCORD_BOT_TOKEN \
  --discord-bot-user-id BOT_USER_ID \
  --discord-intents INTENT_BITSET \
  --discord-profile-id PROFILE_ID \
  --discord-session-id SESSION_ID \
  --discord-cursor "$KEITH_DATA_ROOT/channels/discord-cursor.json" \
  --attachment-root "$KEITH_DATA_ROOT/channel-staging"
```

Run one process per Discord bot account under the same operating-system account
as `agentd`. The bot token is read from the named environment variable. The
cursor and attachment staging paths contain routing and transfer state, never
the token. `docs/discord.md` provides the full Discord operator journey.

### Normalized JSON lines

Without any `--discord-*` options, the executable reads one serialized
`keith_channel_core::RoutedInbound` value per line and emits one JSON report per
line:

```bash
bin/channel-gateway \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --reconnect-attempts 5 < inbound.jsonl
```

The producer must supply the exact profile ID, session ID, external account,
conversation, sender, stable message ID, timestamp, intent, text, and bounded
attachments. This mode is a normalized ingestion seam; it does not authenticate
an upstream platform or send provider-specific replies.

## Architecture and data flow

```text
provider event
  -> provider authentication and size checks
  -> adapter normalization and capability validation
  -> account-scoped replay/deduplication guard
  -> fair bounded gateway queue
  -> native AgentConnection to agentd
  -> profile/session prompt, steer, or cancellation
  -> durable response delivery outbox
  -> exact (channel, external_account) claim
  -> adapter operation and provider receipt
```

The routing identity is `(channel, external_account, conversation, thread)`.
The gateway must never select a profile from message text. Delivery claims are
partitioned by both channel and external account so one bot or workspace cannot
consume another account's reply.

The multi-account supervisor stores account configuration, lifecycle, queue
state, reconnect cursors, credential generation, notification budget, health,
and safe errors. It schedules accounts fairly, persists webhook replay IDs,
isolates adapter workers, and can pause, resume, replace, test, rotate, or remove
one account without stopping every channel.

## Profiles, credentials, approvals, and security

- A configured account belongs to one profile and declares its account ID,
  credential references, scopes, ingress mode, and capability limits.
- Credential values belong in the encrypted credential store or a specifically
  named process environment variable. Browser projections contain references
  and safe state only.
- Webhooks must be authenticated over the original bytes before JSON parsing.
  Freshness and durable replay IDs are part of admission.
- Adapter-returned text, names, files, URLs, receipts, and errors remain
  untrusted. They cannot grant profile, tool, or approval authority.
- Account-changing operations require an `IntegrationMutation` with an exact
  target, profile, capability, risk, idempotency key, revision, cancellation
  identity, audit correlation, and valid approval envelope.
- Legacy channel commands may list and inspect. Connect, configure, test,
  pause, resume, rotate, and remove are rejected unless they arrive through the
  full integration-authority path.
- Attachment staging is bounded and digest checked. Discord downloads are
  restricted to approved Discord CDN endpoints.

## Failure and recovery

- Stable provider message IDs are deduplicated before submission. Queue and
  per-session bounds apply backpressure instead of accepting unbounded work.
- Retryable, reconnect, rate-limit, permanent, stale-cursor, cancellation, and
  uncertain-acknowledgement outcomes remain distinct.
- Rate limits retain their retry deadline. Account workers can restart without
  blocking healthy accounts, and fair scheduling prevents one busy account from
  monopolizing the gateway.
- Durable cursors advance only after the corresponding inbound work is safely
  admitted. A restart may replay uncommitted input; idempotent command identity
  prevents that retry from silently becoming a second turn.
- State-changing sends with an uncertain acknowledgement are not reported as a
  confirmed success. The durable delivery outbox retains failed work according
  to its classified retry policy.
- Removing an account withdraws its worker, replay identities, routes, and
  delivery partition. Revocation and credential rotation are explicit
  lifecycle operations.

## Current limitations

- No external account is qualified by the current evidence. Supplying a token
  later does not retroactively qualify the live journey.
- The standalone gateway executable directly operates only Discord or normalized
  standard-input JSONL. Slack, Telegram, WhatsApp, Teams, Google Chat, email,
  and Matrix need composition with their webhook/polling ingress and supervisor.
- Capabilities differ materially by provider. Consult the capability projection
  rather than assuming parity.
- The Web integration surface can inspect daemon-owned lifecycle state, but
  setup and destructive actions still require a trusted exact approval path.

## Validate changes

Focused channel validation is:

```bash
cargo test -p keith-channel-core -p keith-channel-adapters -p keith-channel-gateway --locked
cargo clippy -p keith-channel-core -p keith-channel-adapters -p keith-channel-gateway \
  --all-targets --no-deps --locked -- -D warnings
```

The ignored adapter tests exercise credential-independent real loopback HTTP
journeys:

```bash
cargo test -p keith-channel-adapters --locked -- --ignored
```

Use the repository's disposable Cargo target convention. A real-account
qualification must additionally prove authentication, inbound and outbound
delivery, duplicates, rate limiting, attachments, restart/resume, revocation,
denied routes, and redacted diagnostics for that exact account with owner
authorization. Update `evidence/channels/qualification.json`; do not replace a
blocked status with a local-loopback result.

## Key source locations

- `crates/channel-core/src/lib.rs` — shared v1/v2 contracts, conformance, queue,
  reconnect, and native agent connection
- `crates/channel-adapters/src/lib.rs` — built-in catalog, Discord, and JSONL adapter
- `crates/channel-adapters/src/{slack,telegram,whatsapp,teams,google_chat,email,matrix}.rs`
  — provider adapters
- `apps/channel-gateway/src/main.rs` — Discord and standard-input executable
- `apps/channel-gateway/src/lib.rs` — multi-account supervision, replay, routing,
  worker isolation, and delivery partitions
- `apps/channel-gateway/tests/supervisor.rs` — fairness, restart, rotation,
  replay, isolation, and lifecycle tests
- `crates/channel-adapters/tests/` — catalog and provider contract journeys
- `crates/protocol/src/lib.rs` — account commands and client projections
- `crates/daemon-core/src/integrations.rs` — profile and approval enforcement
- `evidence/channels/qualification.json` — current qualification truth
