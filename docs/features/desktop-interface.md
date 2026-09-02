# Desktop interface and lifecycle

`agent-desktop` is Keith's local installation and process-lifecycle shell. The
current desktop experience launches the authenticated Web interface in the
operating system's browser; it is not yet a separate native chat window. Its
main responsibilities are safe first-run state, signed release activation,
daemon and Web supervision, browser handoff, backup/restore, rollback, and
explicit uninstall scopes.

## Supported workflows

| Workflow | Command |
| --- | --- |
| Initialize platform-default state | `agent-desktop setup-default [ORIGIN]` |
| Initialize explicit state and data roots | `agent-desktop setup STATE_ROOT DATA_ROOT [ORIGIN]` |
| Inspect non-secret desktop settings | `agent-desktop settings STATE_ROOT` |
| Run the active signed release | `agent-desktop serve STATE_ROOT WORKSPACE_ROOT WEB_BIND [LOGIN_SECRET_ENV] [CREDENTIAL_KEY_ENV]` |
| Open a safe local application route | `agent-desktop open ORIGIN [PATH]` |
| Back up data and notifications | `agent-desktop backup STATE_ROOT` |
| Restore a verified backup | `agent-desktop restore BACKUP TARGET_DATA_ROOT` |
| Hash a release tree | `agent-desktop digest-release RELEASE_DIRECTORY` |
| Verify a release | `agent-desktop verify-release RELEASE_DIRECTORY EXPECTED_PUBLIC_KEY_HEX` |
| Stage and activate an update | `agent-desktop update STATE_ROOT RELEASE_DIRECTORY EXPECTED_PUBLIC_KEY_HEX` |
| Return to the previous complete version | `agent-desktop rollback STATE_ROOT` |
| Preview an uninstall | `agent-desktop uninstall-plan STATE_ROOT DATA_CHOICE` |
| Execute the exact previewed uninstall | `agent-desktop uninstall STATE_ROOT DATA_CHOICE CONFIRMATION` |

## First install and launch

First verify a release with a public key obtained through an independent trusted
channel. Do not rely on a new release's copy of its own public key as the trust
root:

```bash
bin/agent-desktop verify-release \
  /absolute/path/to/release \
  EXPECTED_PUBLIC_KEY_HEX
```

Initialize state, activate the release, and start it:

```bash
state_root=/absolute/path/to/keith-state
data_root=/absolute/path/to/keith-data
workspace_root=/absolute/path/to/workspace

bin/agent-desktop setup \
  "$state_root" \
  "$data_root" \
  http://127.0.0.1:7341

bin/agent-desktop update \
  "$state_root" \
  /absolute/path/to/release \
  EXPECTED_PUBLIC_KEY_HEX

export KEITH_WEB_LOGIN_SECRET='replace-with-a-long-random-password'
export KEITH_CREDENTIAL_KEY='64_HEXADECIMAL_CHARACTERS'
bin/agent-desktop serve \
  "$state_root" \
  "$workspace_root" \
  127.0.0.1:7341
```

In another terminal, open the local application:

```bash
bin/agent-desktop open http://127.0.0.1:7341 /
```

The default secret-environment names for `serve` are
`KEITH_WEB_LOGIN_SECRET` and `KEITH_CREDENTIAL_KEY`. Supplying alternate names
changes which variables are read; secret values are never command arguments.
Send `SIGINT` or `SIGTERM` to the supervisor for an orderly shutdown.

See [Install and lifecycle](../installation.md) for provider setup and the
complete release procedure.

## State ownership

Desktop state and agent data are deliberately separate:

- `STATE_ROOT/desktop.json` contains the installation ID, schema version,
  absolute state/data roots, daemon socket, loopback Web origin, and creation
  time.
- `STATE_ROOT/updates` contains complete version directories, the active
  pointer, and the pinned release-signing public key.
- `STATE_ROOT/crashes` contains bounded, redacted process reports and captured
  child stderr.
- `STATE_ROOT/notifications` contains desktop notification records.
- `STATE_ROOT/backups` contains atomic, digest-verified backups.
- `DATA_ROOT` contains Keith's durable profile, session, memory, artifact,
  credential, and runtime data owned by the daemon and its domain crates.

Settings are validated on every load. State and data roots must be absolute;
symlinked roots and an origin outside loopback are rejected. Reopening an
existing installation is idempotent only when its stored roots and origin match
the request.

## Process and browser architecture

```text
agent-desktop
  -> re-verify active signed release and packaged build reports
  -> refuse an occupied endpoint, then start agentd
  -> negotiate AgentConnection and list sessions as a readiness probe
  -> refuse an occupied listener, then start agent-web on matching loopback origin/port
  -> operating-system browser handoff
  -> authenticated Web interface
```

`serve` resolves `agentd`, `agent-worker`, and `agent-web` only from the active
version's `bin` directory. It validates that the packaged Web export is complete
before starting any child. The daemon is started first and probed through a real
protocol hello and session-list command. The Web process is then started with a
minimal cleared environment containing only the two named secrets.

The browser handoff accepts only a loopback `http`/`https` origin and a relative
absolute-path route. Network-path references, `..`, credentials, query strings,
and fragments are rejected before calling `xdg-open`, `open`, or the Windows
URL handler.

## Authentication and authority boundaries

- The desktop process does not bypass Web authentication. The user still signs
  in with the configured Web login secret.
- The desktop layer does not select or broaden a profile. Profile and session
  authority is negotiated after browser login and remains owned by `agentd`.
- The credential encryption key is passed only to the supervised daemon and Web
  process by its named environment variable.
- The library can reuse a socket only when it completes the real
  AgentConnection readiness probe and its caller explicitly permits reuse. The
  `serve` command does not permit reuse; an existing socket or Web listener
  fails closed.
- Desktop settings never contain provider credentials or the Web password.
- The update public key is the Keith release publisher key. It is separate from
  operating-system application-signing certificates used by Apple, Microsoft,
  or Linux package channels.
- Uninstall requires the installation-specific confirmation from
  `uninstall-plan`; it never removes paths outside the configured state and data
  roots.

## Updates, rollback, and recovery

`update` verifies the release signature, target platform, exact manifest,
packaged daemon/worker build reports, and version. It copies into a private
staging directory, verifies the copy again, atomically moves the complete
version into place, and only then updates `active.json`. The first accepted key
is pinned; a different later publisher key is rejected.

```bash
bin/agent-desktop update \
  "$state_root" \
  /absolute/path/to/new-release \
  EXPECTED_PUBLIC_KEY_HEX

bin/agent-desktop rollback "$state_root"
```

Rollback re-verifies the retained prior version before switching the active
pointer. It does not copy individual binaries between releases.

If an owned daemon or Web child exits, the supervisor records a bounded redacted
crash report and exits instead of leaving a silent partial stack. On normal
shutdown it stops Web first, then gives the daemon time to drain workers before
forcing termination. Durable conversation recovery remains the daemon's job.

## Streaming, reconnect, and failure behavior

The desktop supervisor does not translate or proxy conversation events. Once
the browser is open, command streaming and session reconnect use `agent-web`'s
SSE and WebSocket paths described in [Web interface](web-interface.md). The
desktop process owns only child readiness and lifecycle.

There is no silent child-restart loop. If an owned daemon or Web child exits,
`serve` persists the bounded crash report and exits non-zero so a user service
manager can apply an explicit restart policy. On the next start, `agentd`
recovers durable sessions and the Web client requests an authoritative
snapshot. Failure to negotiate the daemon protocol, an occupied endpoint, a
missing secret environment, incomplete Web assets, or an unverified release
stops startup before the browser is opened.

## Backup, restore, and uninstall

Stop the service before taking a filesystem backup:

```bash
backup_path="$(bin/agent-desktop backup "$state_root")"
bin/agent-desktop restore \
  "$backup_path" \
  /absolute/path/to/empty-restored-data
```

Backup records digests for the copied data and notification trees. Restore
revalidates the manifest and both digests, rejects tampering and symlinks, and
requires an empty target.

Preview removal before executing it:

```bash
bin/agent-desktop uninstall-plan "$state_root" keep-user-data
bin/agent-desktop uninstall \
  "$state_root" \
  keep-user-data \
  'REMOVE INSTALLATION_ID'
```

The data choices are:

- `keep-user-data`: remove installed release versions only;
- `remove-runtime`: also remove crash/notification state, socket, and transient
  runtime data while retaining durable user data; and
- `remove-everything`: remove the configured state and data roots.

Native credential-manager entries are a separate data class and are not
silently removed by deleting files.

## Current limitations and status

- The current desktop binary is a CLI lifecycle supervisor with browser
  handoff, not a native GUI shell, tray application, or embedded WebView.
- `setup` does not install an operating-system service or package; service
  registration belongs to packaging/deployment tooling.
- `update` consumes an already downloaded release directory. It does not fetch
  releases or discover versions over the network.
- The notification center and allowed-root file-selection primitives exist in
  the library, but the current command-line entry point does not expose a
  complete desktop notification or file-picker UI.
- Reusing a healthy Web listener is a reachability check; use the complete
  authenticated browser journey for release qualification.

## Validate changes

Focused coverage lives in the `keith-agent-desktop` library and integration
tests:

- `apps/agent-desktop/tests/lifecycle.rs` exercises real child startup,
  existing-daemon attachment, crash reporting, restart, and graceful stop;
- `apps/agent-desktop/tests/lifecycle_commands.rs` executes backup, restore,
  signed update, rollback, and uninstall through the real binary; and
- `apps/agent-desktop/tests/platform_startup.rs` checks the packaged startup
  connection and owned-process shutdown.

For a distributable build, also verify the signed package from outside the
release, start `serve`, sign in through a real browser, complete a provider turn,
restart, roll back once, and confirm the durable session returns.

## Key source locations

- `apps/agent-desktop/src/main.rs` — command-line lifecycle and `serve` assembly
- `apps/agent-desktop/src/lib.rs` — state, supervision, browser handoff, update, backup, and uninstall policy
- `apps/agent-desktop/tests/lifecycle.rs` — real-process lifecycle coverage
- `apps/agent-desktop/tests/lifecycle_commands.rs` — executable command journey
- `apps/agent-desktop/tests/platform_startup.rs` — packaged startup journey
- `crates/platform/src/lib.rs` — native state/data path discovery
- `crates/release/src/lib.rs` — signed release verification
- `apps/agent-web/` — the browser experience opened by the desktop shell
