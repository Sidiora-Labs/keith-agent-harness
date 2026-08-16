# Install and lifecycle

Keith Agent releases are versioned directories whose signed manifest covers every executable and supporting asset. Verify `release-manifest.sig` against `release-public-key.hex` before installation. Published archives also carry a Sigstore bundle tied to the release workflow identity.

## Install and first run

1. Extract the archive into a new version directory. Do not merge it over an older release.
2. Verify `release-manifest.sig`, then verify every file size and SHA-256 digest listed by `release-manifest.json`.
3. Run `bin/agentd --build-info` and `bin/agent-worker --build-info`; confirm the build ID, protocol version, storage schema, and enabled features match the manifest.
4. Initialize desktop settings with `bin/agent-desktop setup STATE_ROOT DATA_ROOT http://127.0.0.1:7341`.
5. Configure a provider with the authenticated web settings page or the environment-only CLI flow below. Credentials do not belong in shell history, the release, or the data directory.
6. Start `agentd` first, then a TUI or `agent-web`. Keep long-running processes attached to the operating system's user-service manager so stop and restart signals are delivered cleanly.

The release contains `bin/`, `web/`, `builtins/`, `providers/providers.json`, `schemas/`, a CycloneDX SBOM, and a license report. User-created sessions, memory, credentials, logs, and backups are never stored inside the release directory.

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

## Start and stop

Run `agentd --data-root DATA_ROOT --socket ENDPOINT --worker-executable RELEASE/bin/agent-worker --workspace-root WORKSPACE` as the user service. Run `agent-web` against the same endpoint. Stop the web process first and send the daemon its normal termination signal; the daemon drains and stops its workers before exiting. Abrupt process termination is recovered from durable state on the next start.

## Backup and restore

Stop the service before a filesystem backup. The desktop lifecycle command copies the configured data root and notification state into `STATE_ROOT/backups` and prints the new backup path:

```sh
BACKUP_PATH="$(bin/agent-desktop backup STATE_ROOT)"
bin/agent-desktop restore "$BACKUP_PATH" /absolute/path/to/empty-restored-data
```

Restore only into an empty data root, point desktop settings to it, and start the same or a schema-compatible release. Provider credentials remain in the native credential store and must be restored separately by that store's supported mechanism. Move any backup that must survive `remove-everything` outside `STATE_ROOT` before uninstalling.

## Update and rollback

Verify the new release signature and manifest before staging it. Stage it as a complete new version, stop the service, atomically activate it, and restart. The lifecycle executable hashes the complete release tree before copying it:

```sh
RELEASE_DIGEST="$(bin/agent-desktop digest-release /absolute/path/to/new-release)"
bin/agent-desktop update STATE_ROOT 0.2.0 /absolute/path/to/new-release "$RELEASE_DIGEST"
bin/agent-desktop rollback STATE_ROOT
```

Keep the immediately previous version. If readiness or compatibility checks fail, stop the service, run `rollback`, and restart. Never copy individual binaries across active versions.

## Uninstall and data choices

The uninstall plan presents exact paths and the installation-specific confirmation phrase:

```sh
bin/agent-desktop uninstall-plan STATE_ROOT keep-user-data
bin/agent-desktop uninstall STATE_ROOT keep-user-data 'REMOVE INSTALLATION_ID'
```

`keep-user-data` removes only installed releases. `remove-runtime` also removes runtime state while retaining personal data. `remove-everything` removes the configured state and data roots. Native credential-store entries are a separate documented data class and must be removed through the authenticated settings flow or the operating system credential manager. No files are intentionally written outside the selected release, state, data, backup, and native credential-store locations.
