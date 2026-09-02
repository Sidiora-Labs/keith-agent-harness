# Tools, workspaces, and artifacts

Keith performs work through typed tools rather than unrestricted access to the
daemon host. Tool definitions describe their input schema, side effects,
confirmation requirement, time limit, output limit, and whether parallel
execution is safe. The worker then executes an admitted call inside the
profile's workspace and resource boundaries.

Files that should outlive one tool response can be stored as scoped artifacts.
This keeps large or binary results out of the conversation while preserving a
digest, bounded preview, provenance, and lifecycle controls.

## What you can do

- List, read, search, create, edit, rename, copy, and delete files inside the
  authorized workspace.
- Run allowed processes with literal argument vectors, a minimal environment,
  time and memory ceilings, output bounds, and process-tree cancellation.
- Fetch approved Web resources, use browser/computer capabilities, or run the
  guest kernel when those services and profile rules are enabled.
- Use memory, skill, commitment, plan, review, and refinement tools registered
  by the local runtime.
- Make enabled MCP and plugin tools available under the profile's tool limits.
- Inspect tool activity and artifact references from Web and terminal clients.
- Export, archive, retain, or delete artifacts within their profile and session
  scope.
- Preserve versioned personal-workspace files and receive a merge proposal when
  a human/agent edit conflict prevents a safe write.

The actual tool list is runtime- and profile-dependent. A schema appearing in a
client does not prove that the tool is allowed, ready, or available for the
current call.

## Workspace layout and configuration

The daemon receives the working directory through its `--workspace-root`
option. Choose a dedicated, inspectable directory containing only the material
Keith is meant to access. In the Docker setup, `KEITH_WORKSPACE` is mounted at
`/workspace`.

A personal workspace can contain:

```text
AGENT.md
USER.md
RULE.md
MEMORY.md
memory/daily/
state/
knowledge/
skills/
artifacts/
backups/
.keith/
```

These names have different authority. User-authored rules and profile material
are not interchangeable with generated state, retrieved knowledge, or tool
output. The workspace service enforces ownership and conflict rules rather than
letting a model decide which file is authoritative.

Current personal-workspace defaults cap a single file at 16 MiB, total entries
at 100,000, and total data at 512 MiB, with a 250 ms watcher debounce. Artifact
defaults separately cap an individual artifact at 256 MiB, its inline preview
at 4 KiB, and the number of artifacts in one root session tree at 100,000.

See [Install and run Keith](../installation.md) for data-root and Docker
configuration. Tool and workspace policy should be narrowed for the deployment
rather than inferred from what happens to be mounted on the host.

## Architecture and data flow

```text
model proposes typed tool call
             |
 schema + installation/profile policy
             |
 confirmation decision and readiness check
             v
       leased worker process
        /              \
workspace capability   bounded external adapter
        \              /
          typed outcome
       inline preview or artifact spill
             |
 durable session event and client projection
```

`tool-core` owns discovery, schemas, policy combination, confirmation mode,
readiness, retries, cancellation, batches, and typed outcomes. The strictest of
installation and profile policy wins. Input is validated before any
confirmation prompt so malformed arguments cannot masquerade as an approval
request.

`tool-runner` owns filesystem capabilities and restricted process execution.
Paths are resolved under an opened workspace capability with traversal,
device, symlink, preimage, atomic-write, and size checks. Process arguments are
passed as an argument vector and are not reparsed as shell text.

The daemon does not execute the tool directly. It leases work to a worker,
records the request and outcome, and projects progress to clients. Safe read
calls may run in a bounded parallel batch; state-changing calls retain ordering
and effect tracking.

## Workspace edits and conflicts

Workspace writes use an expected revision token and content digest. If a human
changes the file after Keith read it, the later agent edit returns a merge
proposal rather than overwriting the human version. Writes are atomic, and
version snapshots support inspection and restoration.

Textual semantic files must be valid UTF-8. The artifacts directory permits
bounded binary files because its contents are handled through artifact metadata
rather than treated as instructions or workspace policy.

The workspace watcher reports external changes after debounce. A reported file
change can create useful context or an awareness event, but its content remains
untrusted until the normal runtime evaluates it.

## Artifacts

An artifact is scoped to a profile, root session tree, and session. It records a
source such as Tool, Kernel, Child, or User; a digest and byte length; lifecycle
and retention state; and a bounded preview where appropriate.

Large tool or kernel output can spill into an artifact instead of exhausting a
model context or protocol frame. Child sessions may return artifact references
to a parent without copying the complete payload into the conversation.

Artifact reads validate the expected scope, digest, and length. Export is an
explicit operation. Archive and retention state are separate from deletion so
clients can represent the actual lifecycle.

## Authority and security

- Tool permission is the strictest result of installation policy, profile
  policy, delegation, and the tool's declared behavior: allow, confirm, or deny.
- Child sessions and plugins may receive a narrower tool set but cannot widen
  their parent's authority.
- Filesystem calls are rooted capabilities. Absolute paths, traversal, unsafe
  symlinks, device files, and path swaps are rejected.
- Restricted processes receive a minimal environment and an explicit executable
  allowlist. Arguments are not fed through an implicit shell parser.
- Required isolation fails closed. The runtime does not silently downgrade an
  untrusted process to unrestricted host execution.
- Network, browser, MCP, plugin, and fetched content are capability-bearing
  untrusted inputs and do not decide approval or workspace policy.
- Artifact IDs do not bypass profile and session-tree checks. Export must not
  turn an internal reference into public access.
- Credential values remain in the credential service. Tools receive only the
  specific secret binding their adapter is authorized to use.

The seeded local profile permits a useful set of built-in tools. That default
is not a promise that every state-changing built-in produces an interactive
confirmation in every client. Operators should review the effective profile
rules and use a workspace whose changes are inspectable and recoverable.

## Failure and recovery

- Schema-invalid or denied calls fail before execution.
- Readiness checks are cached for a bounded period, and retries are limited to
  failures that the tool declares safe to repeat.
- A failed read-only call can be retried under policy. A state-changing call
  whose process or connection disappears is recorded with an unknown effect and
  is not automatically replayed.
- Timeouts, output floods, memory limits, and cancellation terminate the entire
  owned process tree rather than only the immediate child.
- Large output spills to an artifact with a bounded preview. Failure to store
  the complete output is reported instead of presenting the preview as complete.
- Atomic writes and preimage tokens prevent partial replacement and detect
  external edits. Conflicts produce merge information.
- Artifact creation validates digest and length, and temporary state is replaced
  atomically. Scope or corruption failures are explicit.
- Workspace and artifact state can reconstruct after daemon restart; external
  side effects remain governed by their recorded known or unknown outcome.

## Current limitations and status

- The restricted runner reduces host exposure but is not a universal container
  boundary. Its guarantees depend on the isolation facilities available on the
  host and the policy chosen for the call.
- A mounted workspace defines the maximum filesystem surface; good deployment
  practice is still to mount the smallest useful directory and keep it under
  version control or backup.
- Web fetch, browser, guest kernel, MCP, plugins, and connected apps have their
  own availability and security constraints. Enabling a service group does not
  make every external endpoint safe or configured.
- An artifact records bytes and provenance; it does not certify that generated
  content is correct or safe to execute.
- Focused tool-manager, filesystem race, process-limit, workspace conflict,
  artifact, and restart tests exist. Release-wide security and packaged-runtime
  qualification remain separate work.

## Validate changes

Changes in this area should cover:

- discovery, schema validation, strictest-policy resolution, confirmation,
  readiness, retries, cancellation, and safe parallel reads;
- traversal, absolute path, device file, symlink and path-swap races, size
  bounds, preimage conflicts, atomic writes, and restoration;
- literal argv, environment filtering, executable allowlists, timeout, memory,
  output flood, process-tree cancellation, and required-isolation failure;
- known, failed, cancelled, and unknown side-effect outcomes across restart;
- artifact scope, digest, length, preview, spill, child reference, export,
  archive, retention, deletion, and profile isolation;
- the real daemon-to-worker lease path and client tool-activity projection;
- hostile MCP, plugin, browser, fetched-content, and credential-boundary cases
  when those adapters are affected.

Do not use a helper-only test to claim the complete tool path works. Packaging
or deployment changes should also exercise the worker inside the produced
artifact or image.

## Key source locations

- `crates/tool-core/` — definitions, schemas, policy, confirmation, readiness,
  batching, retries, cancellation, and outcomes
- `crates/tool-runner/` — workspace filesystem capabilities and restricted
  process execution
- `crates/workspace/` — personal-workspace layout, revisions, conflicts,
  snapshots, watchers, and bounds
- `crates/artifacts/` — artifact scope, storage, digest, preview, retention,
  export, and deletion
- `crates/sandbox/` — process and filesystem isolation policy
- `crates/worker-runtime/` — leased execution boundary
- `crates/local-runtime/src/lib.rs` — built-in tool registration and integration
- `crates/plugin-host/`, `crates/mcp/`, and `crates/composio/` — external
  tool adapters and capability boundaries
- `crates/protocol/src/lib.rs` — tool activity and artifact projections
- `apps/agentd/` and `apps/agent-worker/` — daemon/worker orchestration
- `apps/agent-tui/` and `apps/agent-web/` — tool and artifact user experience
- `spec/keith-agent/spec.kvx` — feature requirements and current task status
