# Native platform integration

Keith's `AgentConnection` protocol is the authoritative API. The platform bridge in `agent-web` is a separately authenticated HTTP projection of that protocol for a trusted Centra AI backend. It is distinct from the secondary OpenAI-compatible `/v1` interface used by Open WebUI and similar applications.

## Boundary

The bridge exposes only:

- `GET /platform/v1/health`
- `GET /platform/v1/catalog`
- `GET /platform/v1/capabilities`
- `POST /platform/v1/profiles/{profile}/commands`
- `GET /platform/v1/events/{profile}/{session}`

Commands are native `ClientCommand` values. `agent-web` validates the envelope profile and session scope before sending the command over `AgentConnection`; `agentd` remains the protocol owner and the leased worker remains the only turn executor. The event route checks that the session belongs to the requested profile before opening a bounded server-sent event stream.

Event reconnects carry the last applied `generation` and `sequence` as query parameters. The bridge converts that pair into the native `ResumeCursor`; `agentd` replays missed events or returns its authoritative snapshot when the cursor is stale. The browser retries three consecutive failures with bounded 1-second and 2-second backoff and always resumes from the newest native cursor it has applied.

Set a dedicated random credential of at least 32 bytes:

```sh
export KEITH_PLATFORM_API_KEY="$(openssl rand -hex 32)"
```

If the variable is absent, every `/platform/v1` route is disabled. Loopback is the default. A non-loopback `agent-web` bind with the platform bridge enabled is refused unless `KEITH_PLATFORM_ALLOW_NON_LOOPBACK=true`; that acknowledgement does not provide TLS or network policy.

## Private V1 beta path

The main Railway gateway edge image contains Keith as a fourth sibling runtime. `KEITH_BETA_ENABLED=true` starts its isolated daemon, workers, web bridge, data root, workspace, credentials, provider capability, and health path for that one user service. The image remains the single deployment and update unit; Keith is not deployed as an unrelated Railway service.

The parent platform exposes a same-origin browser BFF at `/keith/v1`. Production traffic uses `NEO_V1_ROUTER_URL`: the BFF presents the verified Supabase session only to the central router, central and shard sign the assigned route, and the edge rewrites `/keith/v1` to loopback `/platform/v1` while replacing every external credential with its Keith-only bearer. Browser authorization and cookies never reach Keith.

The production path requires:

- `NEXT_PUBLIC_KEITH_BETA_ENABLED=true`
- `NEO_V1_ROUTER_URL`, using HTTPS outside loopback
- `KEITH_BETA_PROFILE_ASSIGNMENTS`, a server-only JSON object mapping authenticated Supabase user IDs to an exact profile ID or the reserved `@edge-default` assignment
- `KEITH_BETA_MAX_CONCURRENT_PER_SUBJECT`, optionally setting the BFF cap from 1 through 16 (default 4)

`@edge-default` is valid only on the routed single-tenant path and resolves only when the authenticated subject's assigned edge reports exactly one enabled Keith profile. Missing assignments, zero or multiple enabled profiles, wrong profile/session identifiers, unknown routes, and wrong methods fail closed.

For direct local qualification, `KEITH_PLATFORM_URL` and the matching `KEITH_PLATFORM_API_KEY` target one bridge without the fleet router. For unauthenticated local development only, `KEITH_BETA_DEFAULT_PROFILE_ID` selects the accessible profile. Private HTTP development transport additionally requires `KEITH_PLATFORM_ALLOW_INSECURE_PRIVATE=true` and accepts only loopback or RFC1918 targets.

The BFF verifies the user with Supabase, fails closed when no assignment exists, enforces an exact route and method allowlist, prevents a user from selecting another profile, filters catalog results to the assigned profile, and never forwards a browser-supplied bearer to Keith. The browser sees Keith Beta in the runtime selector only after its assigned catalog loads successfully. Neo remains the default.

Native command bodies and their same-origin BFF projection are limited to 128 KiB. Oversized requests fail with `413 Payload Too Large` before a daemon connection is attempted. The BFF applies the platform edge limit and a separate authenticated-subject bucket, then caps active requests per subject at four by default. `KEITH_BETA_MAX_CONCURRENT_PER_SUBJECT` may set that cap from 1 through 16. The native bridge retains its independent semaphore, so bypassing one admission boundary cannot create unbounded daemon work.

## Current beta surface

The adapter projects durable sessions, committed conversation history, streamed native events, tool state, confirmations, cancellation, capabilities, profile-scoped memory search, same-session branching at an exact committed message, and terminal snapshots into the shared premium Neo chat and Computer. Branch and memory controls are shown only when the daemon advertises their negotiated native features. It deliberately leaves browser attachment staging, artifact retrieval, file browsing, rename, archive, delete, and cross-runtime forks unavailable until each has an exact native operation and ownership-qualified user path.

This source integration is not a deployment or production qualification. Before cohort rollout, run the real daemon/worker/provider/tool path, packaged AgentCore execution path, browser journey, restart and replay tests, profile-isolation probes, clean four-runtime image inspection, and the owner-gated rollout sequence in `spec/keith-beta/spec.kvx`.
