# OpenAI-compatible application interface

Keith Agent's native `AgentConnection` protocol remains the primary API and the only authoritative runtime boundary. The packaged `agent-web` process can additionally expose a deliberately bounded OpenAI Chat Completions interface for existing applications. It translates requests into native session commands; it does not call providers, execute tools, or write session storage itself.

## Enable the interface

Configure a separate random bearer credential in the web service environment. Pass only the environment-variable name to `agent-web`:

```sh
export KEITH_OPENAI_COMPAT_API_KEY="$(openssl rand -hex 32)"
bin/agent-web \
  --bind 127.0.0.1:7341 \
  --origin http://127.0.0.1:7341 \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --asset-root "$PWD/web" \
  --credential-root "$KEITH_DATA_ROOT/credentials" \
  --login-secret-env KEITH_WEB_LOGIN_SECRET \
  --openai-api-key-env KEITH_OPENAI_COMPAT_API_KEY
```

If the named API-key variable is absent, the `/v1` interface remains disabled. The key must contain at least 32 bytes. It is compared through a fixed-size authentication tag and is redacted from configuration diagnostics.

The default loopback binding is suitable when the client runs on the same machine. A non-loopback bind is refused while compatibility is enabled unless `--openai-allow-non-loopback true` is supplied. That acknowledgement does not add TLS: place `agent-web` behind a trusted TLS reverse proxy and network access policy before exposing it beyond the host.

## Supported protocol

The base URL is `http://127.0.0.1:7341/v1`. Every route requires `Authorization: Bearer ...`.

- `GET /models` lists each enabled Keith profile as `keith:<profile-id>`. The response also carries the profile display name.
- `GET /models/{model}` retrieves one projected profile model.
- `POST /chat/completions` accepts ordered text messages with `system`, `developer`, `user`, `assistant`, `tool`, and legacy `function` roles.
- `stream: false` returns an OpenAI-shaped `chat.completion` object.
- `stream: true` returns valid `chat.completion.chunk` server-sent events followed by `data: [DONE]`. Keith currently publishes the committed answer as one content chunk rather than exposing uncommitted provider-token deltas.
- `stream_options.include_usage` emits the final usage-only chunk.

The selected model identifies a Keith profile, not a raw upstream model. The installed profile continues to own provider routing, fallback, persona, rules, tools, confirmations, memory, and resource limits. The convenience model alias `keith` is accepted only when exactly one profile is enabled.

Text Chat Completions with one choice are supported. Bounded client function declarations are accepted when `tool_choice` is absent, `auto`, or `none` so clients such as Open WebUI can attach their ordinary tool catalog, but those declarations are compatibility metadata only: Keith does not execute or return client-defined tool calls, and its profile-owned tools remain active inside the native turn. Required or forced client functions, client tool-call history, image/audio/file parts, multiple choices, log probabilities, and JSON/structured-output modes return explicit OpenAI-shaped `unsupported_feature` errors.

## Durable conversation mapping

Every successful JSON completion includes the native session ID in both the `x-keith-session-id` header and `metadata.keith_session_id`. Streaming completions carry it in the committed, terminal, and usage chunk metadata because HTTP response headers have already been sent before the native turn resolves. A client can send that value back through the same request header or metadata field to resume the exact native session.

For applications that already send a stable conversation identity, Keith recognizes `metadata.chat_id`, `metadata.conversation_id`, `metadata.thread_id`, `x-keith-conversation-id`, `x-openwebui-chat-id`, and `x-thread-id`. It stores only a profile-scoped SHA-256 binding in the native session title; the external user and conversation values are not persisted there. The same binding resumes after `agent-web`, daemon, or worker restart. Without an explicit session or stable conversation binding, each request creates an isolated durable session and submits the complete supplied transcript.

## Open WebUI

In Open WebUI, add an OpenAI connection with:

- URL: `http://host.docker.internal:7341/v1` when Open WebUI runs in Docker, otherwise `http://127.0.0.1:7341/v1`.
- API key: the value configured in `KEITH_OPENAI_COMPAT_API_KEY`.
- Model filter: optional; `/models` discovery returns the canonical Keith profile model IDs.

Docker-to-host access normally requires a non-loopback bind. Use the explicit acknowledgement described above and restrict the port to the Open WebUI host/network. Open WebUI documents that OpenAI-compatible connections use Chat Completions and verify through `/models`: <https://docs.openwebui.com/getting-started/quick-start/connect-a-provider/starting-with-openai-compatible>.

Open WebUI may attach function declarations to ordinary chat requests even when the user did not select a client-side tool. Keith accepts a bounded standard function catalog in automatic mode so those requests continue normally, without granting the remote client new execution authority. Configure the connection to use Chat Completions; the `/v1/responses` endpoint is not currently exposed.

## assistant-ui and OpenAI SDKs

Keep the bearer credential in the server-side route, never in browser code. assistant-ui's AI SDK gateway pattern accepts an OpenAI-compatible `baseURL`:

```ts
import { createOpenAI } from "@ai-sdk/openai";

const keith = createOpenAI({
  baseURL: process.env.KEITH_OPENAI_BASE_URL,
  apiKey: process.env.KEITH_OPENAI_COMPAT_API_KEY,
});

const model = keith(process.env.KEITH_OPENAI_MODEL);
```

Set `KEITH_OPENAI_BASE_URL=http://127.0.0.1:7341/v1` and set `KEITH_OPENAI_MODEL` to an ID returned by `/models`. assistant-ui documents this server-side base-URL substitution at <https://www.assistant-ui.com/docs/integrations/gateways>.

OpenAI SDKs use the same three values: base URL, API key, and projected model ID. The implemented request and response shapes follow OpenAI's Chat Completions reference: <https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/>.

## Curl qualification without a secret argument

Keep the authorization header in an owner-only temporary file so the bearer value does not appear in the process argument list:

```sh
AUTH_HEADER_FILE="$(mktemp)"
chmod 600 "$AUTH_HEADER_FILE"
printf 'Authorization: Bearer %s\n' "$KEITH_OPENAI_COMPAT_API_KEY" >"$AUTH_HEADER_FILE"

curl --fail-with-body --header @"$AUTH_HEADER_FILE" \
  http://127.0.0.1:7341/v1/models

curl --fail-with-body --no-buffer --header @"$AUTH_HEADER_FILE" \
  --header 'Content-Type: application/json' \
  --data '{"model":"keith","messages":[{"role":"user","content":"Reply with READY"}],"stream":true}' \
  http://127.0.0.1:7341/v1/chat/completions

shred -u "$AUTH_HEADER_FILE" 2>/dev/null || rm -f "$AUTH_HEADER_FILE"
```

The examples contain no bearer value. Service managers should inject both the web login secret and compatibility bearer from protected credential files or their native secret facility.
