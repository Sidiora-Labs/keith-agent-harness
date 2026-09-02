# Providers and model routing

Keith presents one runtime contract to the agent loop while supporting several
provider protocols behind it. A profile selects a primary provider and model,
may define fallbacks, and can use separate routes for specialized work such as
classification, summarization, review, or vision.

Routing is explicit and profile-scoped. Keith does not silently turn an
authentication error into a request to a different vendor, and it does not
switch providers after response content has begun.

## What you can do

- Store a provider credential without placing the secret in a profile file or
  client command.
- Choose a provider and model from the terminal or Web settings.
- Override the primary model for a runtime request while retaining the
  profile's policy.
- Configure ordered fallback routes for eligible transient failures.
- Assign optional routes for classification, summarization, review, and vision.
- Point a provider at an approved custom or tenant endpoint when running the
  daemon.
- Inspect the configured provider catalog and route without exposing credential
  values.

The catalog includes native OpenAI Chat, OpenAI Responses/Codex, Anthropic
Messages, Google-compatible, Azure OpenAI, and AWS Bedrock transport families,
as well as compatible services described by the catalog. `provider list` is the
source of truth for the build you are running; a catalog entry does not imply
that credentials, an endpoint, or a particular model are available.

## Configure a provider

Use an environment variable only as the handoff into Keith's credential store:

```bash
export KEITH_DATA_ROOT=/absolute/path/to/keith-data
export OPENAI_API_KEY='replace-me'

bin/agent-cli provider set \
  --provider openai \
  --secret-env OPENAI_API_KEY \
  --data-root "$KEITH_DATA_ROOT"

unset OPENAI_API_KEY
bin/agent-cli provider list --data-root "$KEITH_DATA_ROOT"
```

In the terminal interface, open the Models view or use:

```text
/model PROVIDER
/model PROVIDER MODEL
```

The Web interface exposes the same profile route through Models and Settings.
For installation options and encrypted credential-store setup, see
[Install and run Keith](../installation.md).

An operator can register a non-default endpoint when starting `agentd`:

```text
--provider-base-url provider=https://approved.example/v1
```

This flag is repeatable for different provider names. Keep endpoints in
operator-controlled configuration; do not accept a base URL from model output,
retrieved content, or an untrusted client field.

## Architecture and data flow

```text
profile route + optional request override
                  |
           model registry
       / primary      \ eligible fallback
      v                v
 provider adapter -> normalized ModelEvent stream
                          |
                     agent loop
```

`provider-core` defines the normalized request, event stream, usage, finish
reasons, cancellation, credential wrapper, and error classification. Adapters
translate provider-specific request and streaming formats into that contract.
The model registry validates that referenced providers, models, and credential
references exist and then selects the route for the requested capability.

Streamed text, reasoning, tool-call deltas, completed tool calls, usage, and
finish events remain distinct. This matters to the runtime: receiving bytes is
not the same as receiving a complete, committable assistant response.

Fallback is deliberately narrow. The registry may advance to the next route
only when the error class permits it and the failed provider has emitted no
semantic response delta. This prevents a response from being spliced across
providers.

## Authority and security

- Provider secrets are written to the credential service and represented
  elsewhere by opaque references. The credential wrapper redacts debug output
  and clears its in-memory secret buffer when dropped.
- Model routes are profile-owned. A credential or override from one profile
  cannot be used to service another profile's request.
- Authentication, invalid-request, content-policy, and cancellation errors do
  not authorize fallback.
- A provider adapter parses untrusted network data into bounded typed events;
  those events do not decide tool or approval policy.
- Custom endpoints are an installation-level trust decision. Use TLS and an
  endpoint controlled by the operator or provider.
- Bedrock and other account-scoped services still require the host's normal
  cloud identity and regional configuration; appearing in the catalog is not
  authorization to use them.

## Failure and recovery

- Transient transport and service failures are classified separately from
  authentication, invalid request, content, cancellation, and permanent
  failures.
- An eligible failure before semantic output can move to the next configured
  fallback. Once text, reasoning, or a tool call begins, the route is pinned for
  that turn.
- Cancellation propagates through a cancellation tree to the active adapter and
  stream consumer.
- Malformed or incomplete streams end as typed failures. Partial response text
  is not committed as a final assistant message.
- Provider discovery may be unavailable. A valid explicitly configured model
  can still be used where the adapter supports it.
- Credentials and routes survive client reconnects because they are owned by
  daemon services, not by the UI process.

## Current limitations and status

- Provider and model names, availability, rate limits, and upstream API details
  can change independently of Keith. Verify the current provider account before
  treating a catalog default as deployable.
- Compatibility means the configured service implements the transport contract;
  it does not guarantee identical reasoning, tools, usage accounting, or model
  quality across vendors.
- Fallback is availability handling, not load balancing and not a way to bypass
  an account or content-policy error.
- Some catalog providers require an operator-supplied endpoint and will fail
  explicitly until one is configured.
- The repository includes focused adapter and loopback protocol tests. Live
  provider qualification still depends on external credentials and service
  availability, and release-wide integration is tracked separately.

## Validate changes

Provider changes should exercise the normalized contract and the actual HTTP
stream parser. Cover:

- request serialization, authentication headers, model selection, and usage;
- text, reasoning, and fragmented tool-call streams;
- cancellation before and during a stream;
- malformed frames and incomplete responses;
- the exact error classes that permit or forbid fallback;
- proof that fallback stops after any semantic delta;
- profile and credential isolation;
- OpenAI, Anthropic, compatible endpoint, Responses/Codex, and Bedrock paths
  affected by the change.

The repository tests use real loopback HTTP servers for wire behavior without
claiming that an external provider account was exercised. Record live-provider
checks separately when credentials are available.

## Key source locations

- `crates/provider-core/src/lib.rs` — normalized requests, events,
  credentials, cancellation, and error classes
- `crates/model-registry/src/lib.rs` — profile routes, validation, capability
  selection, and fallback rules
- `crates/provider-adapters/src/lib.rs` — provider HTTP and streaming adapters
- `crates/provider-catalog/src/lib.rs` — provider identities, transports,
  authentication schemes, endpoints, and defaults
- `crates/configuration/src/lib.rs` — model route and thinking-level
  configuration
- `crates/credentials/` — encrypted, profile-scoped credential ownership
- `crates/agent-loop/src/lib.rs` — stream consumption, retry, cancellation,
  and final commit behavior
- `apps/agentd/` and `apps/agent-cli/` — operator configuration and credential
  commands
- `apps/agent-tui/` and `apps/agent-web/` — model-selection clients
- `spec/keith-agent/spec.kvx` — provider requirements and current task status
