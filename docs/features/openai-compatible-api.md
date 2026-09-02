# OpenAI-compatible API

Keith exposes a deliberately bounded subset of the OpenAI Chat Completions API
for applications that already know that protocol. The adapter translates each
request into native Keith session commands. It does not call a model provider,
execute a client-supplied tool, or write session storage directly; `agentd` and
the selected profile remain authoritative.

## Supported endpoints

Use `http://127.0.0.1:7341/v1` as the default base URL.

| Method and path | Behavior |
| --- | --- |
| `GET /v1/models` | List enabled Keith profiles as OpenAI model objects |
| `GET /v1/models/{model}` | Resolve one enabled profile model |
| `POST /v1/chat/completions` | Run one native Keith turn, with JSON or SSE output |

Canonical model IDs are `keith:<profile-id>`. The alias `keith` works only when
exactly one profile is enabled. A model ID selects a Keith profile, not an
upstream provider model: provider routing, persona, installed tools, memory,
approvals, resource limits, and fallback policy all remain owned by that
profile.

## Enable the API

Set a dedicated random bearer credential of at least 32 bytes before starting
`agent-web`. The default environment-variable name is
`KEITH_OPENAI_COMPAT_API_KEY`:

```bash
export KEITH_OPENAI_COMPAT_API_KEY="$(openssl rand -hex 32)"
export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'

bin/agent-web \
  --bind 127.0.0.1:7341 \
  --origin http://127.0.0.1:7341 \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --asset-root "$PWD/web" \
  --credential-root "$KEITH_DATA_ROOT/credentials" \
  --login-secret-env KEITH_WEB_LOGIN_SECRET \
  --openai-api-key-env KEITH_OPENAI_COMPAT_API_KEY
```

If the named key is absent, the routes return `404` with
`compatibility_api_disabled`. All enabled routes require
`Authorization: Bearer ...`. Browser login cookies and the native platform API
key are not accepted.

The server refuses a non-loopback bind while this API is enabled unless
`--openai-allow-non-loopback true` is supplied. That flag only acknowledges the
exposure; it does not provide TLS, firewalling, tenant isolation, or proxy
authentication.

## Discover and call a profile

Keep the bearer out of the process argument list by using a protected header
file:

```bash
auth_headers="$(mktemp)"
chmod 600 "$auth_headers"
printf 'Authorization: Bearer %s\n' \
  "$KEITH_OPENAI_COMPAT_API_KEY" >"$auth_headers"

curl --fail-with-body \
  --header @"$auth_headers" \
  http://127.0.0.1:7341/v1/models

curl --fail-with-body \
  --header @"$auth_headers" \
  --header 'Content-Type: application/json' \
  --data '{
    "model": "keith",
    "messages": [
      {"role": "user", "content": "Reply with READY"}
    ]
  }' \
  http://127.0.0.1:7341/v1/chat/completions

```

The JSON completion contains a normal `chat.completion` choice and usage, plus:

- `x-keith-session-id` in the response headers;
- `metadata.keith_session_id`; and
- `metadata.keith_message_id` for the committed assistant message.

Send the session ID back in `x-keith-session-id` or
`metadata.keith_session_id` to continue the exact durable session.

## Streaming

Set `stream: true` for server-sent `chat.completion.chunk` events:

```bash
curl --fail-with-body --no-buffer \
  --header @"$auth_headers" \
  --header 'Content-Type: application/json' \
  --data '{
    "model": "keith",
    "messages": [{"role": "user", "content": "Explain the current task."}],
    "stream": true,
    "stream_options": {"include_usage": true}
  }' \
  http://127.0.0.1:7341/v1/chat/completions
```

The stream begins with the assistant role, forwards native assistant deltas as
content chunks, and ends with `finish_reason: "stop"` and `data: [DONE]`. If
the native runtime did not expose all committed text as deltas, the adapter
sends the missing committed text before the terminal chunk. With
`include_usage`, a final choices-empty usage chunk is emitted.

Native activity, snapshots, and terminal frames are also projected as
choices-empty chunks under `metadata.keith_event`. Clients that understand the
extension can display Keith's live work; ordinary Chat Completions clients may
ignore the metadata-only chunks.

The response headers are committed before the native session has been resolved,
so streaming session IDs appear in terminal and usage chunk metadata rather
than `x-keith-session-id`. If an in-progress stream disconnects, there is no
HTTP SSE replay endpoint. Reconnect the application using a stable conversation
binding or a session ID already received; do not assume an interrupted request
did not reach Keith.

Remove the temporary header file after the last request:

```bash
shred -u "$auth_headers" 2>/dev/null || rm -f "$auth_headers"
```

## Durable conversation mapping

For clients that already have stable conversation identifiers, Keith recognizes:

- headers `x-keith-conversation-id`, `x-openwebui-chat-id`, and `x-thread-id`;
- metadata keys `chat_id`, `conversation_id`, and `thread_id`; and
- the optional OpenAI `user` value as an additional namespace.

All supplied binding values must agree and be at most 512 bytes. Keith hashes
the profile, user, and binding into a profile-scoped native session title. The
external identifier and user value are not persisted in clear text there. This
mapping survives `agent-web`, daemon, and worker restart.

When neither an explicit Keith session nor a conversation binding is supplied,
each request creates a new durable session and submits the complete transcript.
For a continuation, client system/developer messages and messages after the
latest client-supplied assistant turn are sent; already committed conversation
history is not echoed back into the same native session.

An explicit session must belong to the selected profile. A mismatch fails with
`403 keith_session_scope_mismatch`.

## Accepted request surface

- One completion choice (`n` omitted or `1`)
- Text content as a string or an array of `type: "text"` parts
- Roles `system`, `developer`, `user`, `assistant`, `tool`, and legacy
  `function`, provided the messages do not contain tool-call history
- `stream` and `stream_options.include_usage`
- Text-only `modalities` and `response_format: {"type": "text"}`
- Up to 64 bounded function definitions as advisory client metadata when
  `tool_choice`/`function_call` is absent, `auto`, or `none`

Client-provided system and developer content remains subordinate to the
installed Keith persona, profile policy, and safety boundaries. Advisory
function declarations do not grant runtime authority and are not executed or
returned as client tool calls. Keith's installed profile-owned tools remain
available under native policy.

## Unsupported operations

The API explicitly rejects:

- `/v1/responses`, embeddings, images, audio, files, batches, fine-tuning, and
  every unregistered OpenAI route;
- multiple choices, log probabilities, JSON/structured-output modes, non-text
  modalities, and non-text content parts;
- required or forced client function execution;
- client tool-call or function-call history; and
- malformed, oversized, or conflicting conversation/session values.

The request-body limit is 128 KiB. The adapter admits at most 16 concurrent
OpenAI-compatible requests in the default server configuration and returns
`429 keith_capacity_exhausted` rather than queuing unbounded work.

## Architecture and authority flow

```text
OpenAI-compatible client
  -> dedicated bearer and bounded Chat Completions request
  -> agent-web request validation and profile/session resolution
  -> native CreateSession or ResumeSession plus SubmitPrompt
  -> agentd and the selected profile's leased worker
  -> native ordered events and authoritative session snapshot
  -> OpenAI-shaped JSON or SSE projection
```

The compatibility layer cannot pick a provider credential, grant a tool, skip
confirmation, or write durable state outside native commands. The requested
model resolves only an enabled profile; explicit sessions are checked inside
that profile; and conversation bindings are hashed with the profile identity.
The dedicated bearer authorizes every enabled profile visible to this
`agent-web` instance, so multi-user deployments need a trusted server-side
assignment layer in front of it. Never embed the bearer in browser code.

## Errors and failure behavior

Non-streaming failures use the OpenAI error envelope with `message`, `type`,
`param`, and `code`. Authentication failures return `401` and a
`WWW-Authenticate` challenge. Native unavailable, protocol, timeout,
conflict/cancellation, resource, and internal error classes are mapped to an
appropriate HTTP status without exposing the daemon error object.

After an SSE response starts, a runtime failure is delivered as an OpenAI error
JSON event followed by `[DONE]`; the HTTP status can no longer change. A
successful response is returned only when the native session snapshot contains
a newly committed final assistant message. An accepted command without such a
message fails with `keith_turn_incomplete`.

## Validate changes

The unit tests in `openai_compat.rs` cover exact bearer checks, unambiguous model
selection, private profile-scoped conversation binding, ordered roles,
continuation de-duplication, bounded advisory tools, explicit rejection of
forced tools, and session metadata in streams. The packaged startup test covers
unauthenticated access, unavailable-daemon errors, advisory and forced tools,
and request-size limits.

For a real integration qualification, start `agentd` with a configured provider
and call model discovery plus both JSON and SSE completions. Verify same-session
continuation, a stable conversation binding across process restart, profile
scope rejection, and the client application's handling of metadata-only activity
chunks. A route/unit test alone does not prove a provider turn.

## Key source locations

- `apps/agent-web/src/server/openai_compat.rs` — compatibility adapter
- `apps/agent-web/src/server.rs` — route registration, configuration, and daemon bridge
- `apps/agent-web/tests/platform_startup.rs` — packaged route and security boundary test
- `crates/protocol/src/lib.rs` — native session and prompt commands
- `crates/daemon-core/src/` — authoritative command execution and session ownership
- `scripts/ci/container-smoke.sh` — packaged HTTP smoke path
