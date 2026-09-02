# Install and run Keith

Keith can run from Docker Compose, a source checkout, or a signed release
archive. All three paths use the same durable daemon, profile/provider model,
Web interface, and terminal client.

For a first local run, use Docker Compose. Use a source checkout when developing
Keith. Use a signed archive when you need an immutable installation with managed
backup, update, rollback, and uninstall behavior.

## Before you start

Keith needs:

- a model-provider credential;
- a directory it may use as its workspace;
- a durable data location; and
- a separate Web login secret.

Keith can run commands, edit files, control a browser, and call configured
services. Start with a workspace you can inspect and restore. Keep the Web/API
listeners on loopback unless you have configured TLS and network policy as
described in [Deployment](deployment.md).

## Option 1: Docker Compose

From the repository root:

```bash
cp .env.example .env
```

Edit `.env`. At minimum, replace `KEITH_WEB_LOGIN_SECRET` and set one provider
credential such as `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`,
`OPENROUTER_API_KEY`, or `GEMINI_API_KEY`. Do not commit the populated file.

Then start the stack:

```bash
./keith up
./keith logs
```

Open <http://localhost:7341> and sign in with the Web login secret. Compose
stores durable state in the `keith-data` volume and mounts `KEITH_WORKSPACE`
from `.env` at `/workspace` inside the container.

Stop the service without deleting its volume:

```bash
./keith down
```

Read [Deployment](deployment.md) before exposing the container outside the
host or moving it to a cloud provider.

## Option 2: Run from source

### Requirements

- Rust 1.93.0 with `rustfmt` and Clippy
- Node.js 22.22.2
- Corepack with pnpm 11.18.0
- Git

Check the local environment and install locked dependencies:

```bash
./keith doctor
./keith setup
```

Choose an explicit development data root and provide one model credential:

```bash
export KEITH_DEV_DATA_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/keith-dev"
export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'
export OPENROUTER_API_KEY='your-provider-key'
./keith dev
```

`./keith dev` imports recognized provider variables into Keith's encrypted
credential store, removes them from the child-process environment, builds the
daemon, worker, Web server, and administration CLI, and opens the local service
at <http://127.0.0.1:7341>. It keeps Cargo output under
`${XDG_CACHE_HOME:-/tmp}/keith-dev/target`, outside the checkout.

The wrapper enables channels, ACP, plugins, connected apps, computers, and
teaching in development. Feature enablement does not configure external
accounts or prove their credentialed journeys.

## Configure or change a provider

The Web Settings page can store a provider credential after authentication.
For a headless or release installation, use `agent-cli` and read the secret from
an environment variable rather than a command argument:

```bash
export KEITH_DATA_ROOT=/absolute/path/to/keith-data
export OPENAI_API_KEY='your-provider-key'

bin/agent-cli provider set \
  --provider openai \
  --secret-env OPENAI_API_KEY \
  --data-root "$KEITH_DATA_ROOT"

unset OPENAI_API_KEY
bin/agent-cli provider list --data-root "$KEITH_DATA_ROOT"
```

The default credential reference is `default`. Provider values are encrypted;
plaintext keys must not be placed in configuration files, shell arguments,
logs, session data, or release directories.

Some providers require an account- or deployment-specific base URL. Pass each
one to `agentd` explicitly:

```bash
bin/agentd \
  --data-root "$KEITH_DATA_ROOT" \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --worker-executable "$PWD/bin/agent-worker" \
  --workspace-root /absolute/path/to/workspace \
  --provider-base-url azure-openai-responses=https://RESOURCE.openai.azure.com/openai/v1 \
  --provider-base-url custom-openai=https://provider.example/v1
```

A provider can remain visible before its credential or endpoint is ready.
Selection then fails explicitly instead of silently substituting another
provider. See [Providers and model routing](features/providers-and-model-routing.md)
for the catalog, transports, model selection, fallback, and current limits.

### Headless credential master key

On a host where the installation cannot create or use its normal local key,
generate a 32-byte master key as 64 hexadecimal characters and inject it from a
protected service environment. Pass only the variable name:

```bash
export KEITH_CREDENTIAL_KEY='64_HEXADECIMAL_CHARACTERS'
export OPENAI_API_KEY='your-provider-key'

bin/agent-cli provider set \
  --provider openai \
  --secret-env OPENAI_API_KEY \
  --data-root "$KEITH_DATA_ROOT" \
  --credential-key-env KEITH_CREDENTIAL_KEY

bin/agentd \
  --data-root "$KEITH_DATA_ROOT" \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --worker-executable "$PWD/bin/agent-worker" \
  --workspace-root /absolute/path/to/workspace \
  --credential-key-env KEITH_CREDENTIAL_KEY
```

Losing or changing the master key makes the encrypted provider credentials
unreadable. Store and back it up separately from the data root.

## Use the terminal interface

With `./keith dev` running and the same `KEITH_DEV_DATA_ROOT`, attach from a
second terminal:

```bash
CARGO_TARGET_DIR="${XDG_CACHE_HOME:-/tmp}/keith-dev/target" \
CARGO_INCREMENTAL=0 \
  cargo run --quiet -p keith-agent-tui --bin agent-tui -- \
  --socket "$KEITH_DEV_DATA_ROOT/agentd.sock"
```

For a packaged release:

```bash
bin/agent-tui --socket "$KEITH_DATA_ROOT/agentd.sock"
```

The TUI attaches to a durable session and uses the same profile, model, tools,
and history as the Web interface. See [Terminal interface](features/terminal-interface.md)
for commands, keys, pending/streaming states, reconnect, and accessibility.

`Connection refused` means the socket path exists but no daemon is accepting
connections there. Confirm `agentd` is running with the same data root, then
remove a stale socket only after the daemon is stopped.

## Run the Web server directly

Start `agentd` first, then point `agent-web` at its socket and credential root:

```bash
export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'

bin/agent-web \
  --bind 127.0.0.1:7341 \
  --origin http://127.0.0.1:7341 \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --asset-root "$PWD/web" \
  --credential-root "$KEITH_DATA_ROOT/credentials" \
  --login-secret-env KEITH_WEB_LOGIN_SECRET
```

The OpenAI-compatible and native APIs are independently disabled unless their
own bearer keys are configured. See the
[OpenAI-compatible API](features/openai-compatible-api.md) and
[native platform API](features/native-platform-api.md) guides.

## Option 3: Install a signed release

A release is an immutable version directory containing `bin/`, `web/`,
`builtins/`, `providers/providers.json`, `schemas/`, Cargo provenance, a
CycloneDX SBOM, a license report, and a signed manifest. Sessions, memory,
credentials, logs, workspaces, and backups live outside that directory.

### Verify before running

The `release-public-key.hex` inside an archive is a copy of the publisher key,
not an independent trust root. Obtain the expected Ed25519 public key through a
separate authenticated channel.

From an audited source checkout:

```bash
cargo xtask verify-release /absolute/path/to/release EXPECTED_PUBLIC_KEY_HEX
```

From an already trusted Keith installation:

```bash
bin/agent-desktop verify-release /absolute/path/to/release EXPECTED_PUBLIC_KEY_HEX
```

Verification checks the publisher key, signature, exact file set, sizes,
SHA-256 digests, permissions, paths, build identity, and component
compatibility. It rejects unlisted files, duplicate paths, and symlinks. Do not
execute a newly downloaded release's verifier as the only proof of that same
download.

### Initialize and serve

```bash
bin/agent-desktop setup \
  STATE_ROOT \
  DATA_ROOT \
  http://127.0.0.1:7341

export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'
export KEITH_CREDENTIAL_KEY='64_HEXADECIMAL_CHARACTERS'

bin/agent-desktop serve \
  STATE_ROOT \
  /absolute/path/to/workspace \
  127.0.0.1:7341
```

The desktop lifecycle owns the daemon and Web child processes. If either child
crashes, it writes a bounded report under `STATE_ROOT/crashes` and stops rather
than silently leaving a partial stack. Send the supervisor `SIGTERM` or
`SIGINT` for an orderly stop.

## Backup and restore

Stop the managed service before a filesystem backup:

```bash
backup_path="$(bin/agent-desktop backup STATE_ROOT)"
bin/agent-desktop restore "$backup_path" /absolute/path/to/empty-restored-data
```

The backup is assembled atomically with a versioned manifest and data and
notification-tree digests. Restore rejects changed bytes, unsafe paths, and
symlinks, and only promotes into an empty target.

Provider credentials and the credential master key are a separate data class.
Back them up using the configured credential backend's procedure. A data-root
backup without the master key cannot decrypt them.

## Update and rollback

Verify a new release before staging it. Keep versions separate; never copy
individual binaries over an active installation.

```bash
bin/agent-desktop update \
  STATE_ROOT \
  /absolute/path/to/new-release \
  EXPECTED_PUBLIC_KEY_HEX

bin/agent-desktop rollback STATE_ROOT
```

The desktop lifecycle pins the publisher key on first use, verifies before and
after copying, and re-verifies the retained version during rollback. Stop the
service before update or rollback, then restart and confirm readiness and data
compatibility.

## Uninstall and data choices

Preview exact paths and the installation-specific confirmation phrase:

```bash
bin/agent-desktop uninstall-plan STATE_ROOT keep-user-data
bin/agent-desktop uninstall \
  STATE_ROOT \
  keep-user-data \
  'REMOVE INSTALLATION_ID'
```

- `keep-user-data` removes installed release versions.
- `remove-runtime` additionally removes crash reports, notifications, the
  daemon socket, and transient runtime state while retaining durable user data.
- `remove-everything` removes the configured state and data roots.

Move any backup that must survive `remove-everything` outside `STATE_ROOT`
before uninstalling. Credential-backend entries may require separate removal.

## Build a signed release

This is a maintainer workflow, not a normal installation step. Release assembly
requires an explicit build ID and a 32-byte Ed25519 signing seed encoded as 64
hexadecimal characters:

```bash
export KEITH_BUILD_ID='git-COMMIT_OR_RELEASE_BUILD_ID'
export KEITH_RELEASE_SIGNING_KEY='64_HEXADECIMAL_CHARACTERS'
cargo xtask release /absolute/path/to/new-release
unset KEITH_RELEASE_SIGNING_KEY
```

`KEITH_RELEASE_SIGNING_KEY` signs Keith's release manifest. It is not an Apple
Developer ID certificate, Microsoft Authenticode certificate, or desktop app-
store signing identity. Native platform packaging may require those separate
credentials in addition to Keith's manifest signature.

The builder compiles locked release binaries and WebAssembly assets, assembles
the result in a private sibling staging directory, signs and verifies it, runs
the packaged daemon and worker build reports, and only then promotes the
complete directory. Follow [Release qualification](release-qualification.md)
before publication.
