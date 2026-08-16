# Install and lifecycle

Keith Agent releases are versioned directories whose signed manifest covers every executable and supporting asset. The `release-public-key.hex` file inside the release is a copy of the publisher key, not a trust root. Obtain the expected public-key value through an independent authenticated channel and require an exact match while verifying `release-manifest.sig`.

For a source-built release, the build-side verifier accepts the release directory and independently obtained key:

```sh
cargo xtask verify-release /absolute/path/to/release EXPECTED_PUBLIC_KEY_HEX
```

An already trusted Keith installation can verify a downloaded update without a source checkout:

```sh
bin/agent-desktop verify-release /absolute/path/to/release EXPECTED_PUBLIC_KEY_HEX
```

Do not execute a newly downloaded release's own verifier as the only proof of that same release. First installation must be authenticated by the operating-system package channel, an independently obtained verifier, or the build-side command above.

## Install and first run

1. Extract the archive into a new version directory. Do not merge it over an older release.
2. Verify the publisher key, signature, exact payload file set, file sizes, and SHA-256 digests. Verification rejects unlisted files, duplicate paths, symlinks, and unsafe paths.
3. Run `bin/agentd --build-info` and `bin/agent-worker --build-info`; confirm the build ID, protocol version, storage schema, and enabled features match the manifest.
4. Initialize desktop settings with `bin/agent-desktop setup STATE_ROOT DATA_ROOT http://127.0.0.1:7341`.
5. Configure a provider with the authenticated web settings page or the environment-only CLI flow below. Credentials do not belong in shell history, the release, or the data directory.
6. Start `agentd` first, then a TUI or `agent-web`. Keep long-running processes attached to the operating system's user-service manager so stop and restart signals are delivered cleanly.

The release contains `bin/`, `web/`, `builtins/`, `providers/providers.json`, `schemas/`, `provenance/Cargo.lock`, a CycloneDX SBOM, and a license report. The signed manifest records the shared build ID and complete daemon and worker compatibility reports. User-created sessions, memory, credentials, logs, and backups are never stored inside the release directory.

## Produce a release

Release construction requires an explicit non-development build ID and a 32-byte Ed25519 signing seed encoded as 64 hexadecimal characters. Both values must be present before Cargo compiles the build tool so the packaged binaries and signed manifest receive the same build identity:

```sh
export KEITH_BUILD_ID='git-COMMIT_OR_RELEASE_BUILD_ID'
export KEITH_RELEASE_SIGNING_KEY='64_HEXADECIMAL_CHARACTERS'
cargo xtask release /absolute/path/to/new-release
unset KEITH_RELEASE_SIGNING_KEY
```

Construction builds locked release binaries and Rust/WASM assets, assembles everything in a private sibling staging directory, signs and verifies the result, executes packaged daemon and worker build reports, and only then atomically promotes the complete directory. A failed build never promotes a partial release.

## Connect a provider and use the TUI

The default credential reference is `default`. Keith creates an owner-only local master key under the credential root, so desktop and headless installs use the same flow without requiring a secret-service daemon. Configure OpenAI without placing the provider key in a command argument:

```sh
export KEITH_DATA_ROOT=/absolute/path/to/keith-data
export OPENAI_API_KEY='your-provider-key'
bin/agent-cli provider set --provider openai --secret-env OPENAI_API_KEY --data-root "$KEITH_DATA_ROOT"
unset OPENAI_API_KEY
bin/agent-cli provider list --data-root "$KEITH_DATA_ROOT"
```

Start the daemon and attach the TUI:

```sh
bin/agentd --data-root "$KEITH_DATA_ROOT" --socket "$KEITH_DATA_ROOT/agentd.sock" --worker-executable "$PWD/bin/agent-worker" --workspace-root /absolute/path/to/workspace
bin/agent-tui --socket "$KEITH_DATA_ROOT/agentd.sock"
```

For a release managed by the desktop lifecycle, activate it with `update` and run the complete daemon plus authenticated web surface through the signed active version. Both child processes receive the same credential-key reference, and the daemon receives the explicit workspace root:

```sh
export KEITH_WEB_LOGIN_SECRET='a-long-local-login-secret'
export KEITH_CREDENTIAL_KEY='64_HEXADECIMAL_CHARACTERS'
bin/agent-desktop serve STATE_ROOT /absolute/path/to/workspace 127.0.0.1:7341
```

Send the desktop supervisor `SIGTERM` or `SIGINT` to stop web first and drain the daemon and workers. A managed child crash produces a bounded report beneath `STATE_ROOT/crashes` and stops the supervisor instead of silently running a partial stack.

The TUI attaches to the first durable session. Open the Models view to inspect the complete Prime/Cow provider catalog. Send `/model PROVIDER` to select that provider's catalog default, or `/model PROVIDER MODEL` to choose an explicit model. The web Models and Settings selectors expose the same catalog. `bin/agent-cli provider list` prints every provider ID, transport, authentication mode, and conventional environment-variable name.

The same environment-only credential command works for every provider. For example:

```sh
export DEEPSEEK_API_KEY='your-provider-key'
bin/agent-cli provider set --provider deepseek --secret-env DEEPSEEK_API_KEY --data-root "$KEITH_DATA_ROOT"
unset DEEPSEEK_API_KEY
# In the TUI:
# /model deepseek
```

OpenAI-compatible, Anthropic-compatible, Gemini, Azure OpenAI, ChatGPT Codex Responses, GitHub Copilot bearer, and Amazon Bedrock Converse transports are normalized by the runtime. Bedrock uses a scoped `AWS_BEARER_TOKEN_BEDROCK` value. ChatGPT Codex accepts its OAuth access-token JWT through the same write-only credential flow; GitHub Copilot accepts an exchanged Copilot bearer token. Those tokens are never accepted as command arguments.

Account- or deployment-specific providers have no safe global endpoint. Supply each endpoint when starting `agentd`; the flag may be repeated:

```sh
bin/agentd \
  --data-root "$KEITH_DATA_ROOT" \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --worker-executable "$PWD/bin/agent-worker" \
  --workspace-root /absolute/path/to/workspace \
  --provider-base-url azure-openai-responses=https://RESOURCE.openai.azure.com/openai/v1 \
  --provider-base-url custom-openai=https://provider.example/v1
```

Azure OpenAI uses its API key in the `api-key` header. Cloudflare AI Gateway, Cloudflare Workers AI, Google Vertex AI, and custom OpenAI-compatible deployments also require their account-specific base URL. A provider remains visible in clients before its endpoint is configured, but model selection fails explicitly instead of silently substituting another provider.

On a headless machine without a native keyring, generate and persist a 32-byte master key as a 64-character hex value in a protected service environment. Pass only its environment-variable name to every process:

```sh
bin/agent-cli provider set --provider openai --secret-env OPENAI_API_KEY --data-root "$KEITH_DATA_ROOT" --credential-key-env KEITH_CREDENTIAL_KEY
bin/agentd --data-root "$KEITH_DATA_ROOT" --socket "$KEITH_DATA_ROOT/agentd.sock" --worker-executable "$PWD/bin/agent-worker" --workspace-root /absolute/path/to/workspace --credential-key-env KEITH_CREDENTIAL_KEY
```

Losing or changing that master key makes the encrypted provider credentials unreadable.

## Start the web application

Set a separate login secret and point the authenticated local web server at the same daemon and credential store:

```sh
export KEITH_WEB_LOGIN_SECRET='a-long-local-login-secret'
bin/agent-web --bind 127.0.0.1:7341 --origin http://127.0.0.1:7341 --socket "$KEITH_DATA_ROOT/agentd.sock" --asset-root "$PWD/web" --credential-root "$KEITH_DATA_ROOT/credentials" --login-secret-env KEITH_WEB_LOGIN_SECRET
```

Open `http://127.0.0.1:7341`, sign in, and use Settings to configure a provider or Models to change the active model. New chat creates a durable session in the current profile.

`agent-web` can also expose a separately authenticated OpenAI-compatible `/v1` interface for Open WebUI, assistant-ui, OpenAI SDKs, and similar applications. This remains a thin adapter over the primary native `AgentConnection` API. See [OpenAI-compatible application interface](openai-compatibility.md) for enablement, supported behavior, durable session mapping, and network-safety requirements.

## Start and stop

Run `agentd --data-root DATA_ROOT --socket ENDPOINT --worker-executable RELEASE/bin/agent-worker --workspace-root WORKSPACE` as the user service. Run `agent-web` against the same endpoint. Stop the web process first and send the daemon its normal termination signal; the daemon drains and stops its workers before exiting. Abrupt process termination is recovered from durable state on the next start.

## Backup and restore

Stop the service before a filesystem backup. The desktop lifecycle command copies the configured data root and notification state into `STATE_ROOT/backups` and prints the new backup path:

```sh
BACKUP_PATH="$(bin/agent-desktop backup STATE_ROOT)"
bin/agent-desktop restore "$BACKUP_PATH" /absolute/path/to/empty-restored-data
```

Backup construction uses a sibling staging directory and atomically promotes it only after writing a versioned manifest with data and notification-tree digests. Restore revalidates that manifest and both trees, rejects symlinks or modified bytes, and atomically promotes the restored data into an empty target. Notification state remains in the backup for inspection; it is not imported into a different desktop state root.

Restore only into an empty data root, point desktop settings to it, and start the same or a schema-compatible release. Provider credentials remain in the native credential store and must be restored separately by that store's supported mechanism. Move any backup that must survive `remove-everything` outside `STATE_ROOT` before uninstalling.

## Update and rollback

Verify the new release signature and manifest before staging it. Stage it as a complete new version, stop the service, atomically activate it, and restart. The lifecycle executable pins the independently supplied publisher key on first use, rejects key changes, verifies before copying, re-verifies the copied tree, and re-verifies a retained version during rollback:

```sh
bin/agent-desktop update STATE_ROOT /absolute/path/to/new-release EXPECTED_PUBLIC_KEY_HEX
bin/agent-desktop rollback STATE_ROOT
```

Keep the immediately previous version. If readiness or compatibility checks fail, stop the service, run `rollback`, and restart. Never copy individual binaries across active versions.

## Uninstall and data choices

The uninstall plan presents exact paths and the installation-specific confirmation phrase:

```sh
bin/agent-desktop uninstall-plan STATE_ROOT keep-user-data
bin/agent-desktop uninstall STATE_ROOT keep-user-data 'REMOVE INSTALLATION_ID'
```

`keep-user-data` removes only installed release versions. `remove-runtime` additionally removes crash reports, notification state, the daemon socket, and the transient runtime directory while retaining sessions, profiles, memory, artifacts, indexes, schedules, and credentials. `remove-everything` removes the configured state and data roots. Native credential-store entries are a separate documented data class and must be removed through the authenticated settings flow or the operating system credential manager. No files are intentionally written outside the selected release, state, data, backup, and native credential-store locations.
