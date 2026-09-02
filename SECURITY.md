# Security Policy

If you believe you found a security issue in Keith, report it privately first.
Do not open a public issue, Discussion, or pull request containing an unpatched
vulnerability, exploit path, leaked credential, or security-sensitive proof of
concept.

Keith is local-first agent infrastructure that can execute commands, edit files,
control a browser, and call external services. Useful reports demonstrate a
current, reproducible bypass of an authentication, authority, approval,
credential, sandbox, storage, or release-integrity boundary.

## Supported versions

| Version | Security support |
| --- | --- |
| Latest GitHub Release, when one has been published | Supported |
| Current default branch | Supported for development |
| Older releases, untagged builds, and arbitrary commits | Upgrade may be required |

Keith is currently pre-1.0. Security fixes normally target the current default
branch and the latest published release; backports are evaluated case by case.
The presence of a tag or artifact is not itself a security-support promise.

## Report a vulnerability

Submit a private report through GitHub's
[security advisory form](https://github.com/machinecity/keith-agent/security/advisories/new).
If the form is unavailable, contact the repository owners privately through the
[Machine City organization](https://github.com/machinecity). Do not move a
private report into a public channel without maintainer agreement.

Keith does not currently operate a paid bug-bounty program.

### What to include

Make the report easy to reproduce and route:

- A concise description of the vulnerability and affected boundary.
- The affected version, commit SHA, component, operating system, and deployment
  model.
- Minimal reproduction steps or a proof of concept against a current revision.
- Expected behavior, observed behavior, and demonstrated impact.
- Whether the issue requires operator access, a configured plugin or skill,
  channel access, API access, or control of local files or environment values.
- Any mitigation or remediation advice you can provide.

Use synthetic or disposable values. Never attach real provider keys, login
secrets, API bearer tokens, signing keys, credential stores, session databases,
personal data, or unredacted execution traces.

### What to expect

Maintainers will acknowledge a reproducible report as soon as practical,
determine its scope and severity, and coordinate remediation and disclosure.
There is no guaranteed response or repair SLA while the project is pre-release.
Please allow time for investigation before publishing details that could put
users at risk.

With the reporter's permission, valid findings may be credited in the advisory
or release notes.

## Security model

The following boundaries guide both deployment and vulnerability triage.

### Operator and host

- A Keith deployment has a trusted operator and host-administration boundary.
- Anyone who can replace Keith binaries, modify its protected data root, inject
  process environment values, or control its service manager is already inside
  that host boundary.
- Profile and session scoping protect normal data ownership and command routing;
  they are not a substitute for separate operating-system or host isolation
  between mutually untrusted administrators.

### Models and external content

- Models are not trusted principals. Prompts, channel messages, fetched pages,
  files, tool output, and retrieved memory may contain hostile instructions.
- Security decisions belong to deterministic authentication, typed authority,
  approval, credential, sandbox, resource, evaluator, and promotion boundaries.
- Prompt injection is security-relevant when it crosses one of those boundaries,
  not merely because the model follows an unwanted instruction.

### Tools, computer use, and integrations

- Tools and computer-use sessions act with the authority explicitly available
  to their worker, lease, profile, workspace, and approval envelope.
- Connected-app writes and other external mutations may require confirmation.
  A missing, forged, replayed, cross-profile, or bypassed approval is in scope.
- Channel signatures, account ownership, durable delivery partitions, and
  external-principal mapping are security boundaries.

### Plugins, skills, and MCP servers

- WASI plugins are capability-scoped, but installed packages remain code chosen
  by the operator. Review their source, dependencies, requested capabilities,
  and state paths.
- Skills can influence model behavior, and MCP or Composio servers can expose
  external authority. Install and connect only components you trust.
- Malicious behavior entirely within authority knowingly granted to a trusted
  component is not, by itself, a sandbox bypass.

### APIs and network exposure

- The Web login, OpenAI-compatible API, and native platform API use separate
  credentials and must remain separately configured.
- Possession of an API credential grants the authority documented for that API;
  it is not a user-scoped identity token unless the integration layer explicitly
  establishes and verifies that identity.
- Loopback is the safe default. Non-loopback deployments require TLS, strong
  secret management, origin configuration, ingress restrictions, and network
  policy supplied by the operator.

### Self-improvement and releases

- Harness candidates may not change protected evaluators, benchmarks, approval
  rules, security policy, runtime authority, or promotion gates.
- Candidate isolation, signed manifests, independent evaluation, canary
  observation, rollback, and the independently obtained release public key are
  security boundaries.
- A public key included inside a downloaded release is not an independent trust
  root. Follow [the installation guide](docs/installation.md) when verifying a
  release.

The [self-evolution guide](docs/features/self-evolution.md) explains candidate
authority and current qualification limits. The
[release qualification matrix](docs/release-qualification.md) defines the
stronger artifact-wide bar; focused self-evolution tests do not imply that bar
has passed.

## In scope

Examples of useful security reports include:

- Authentication or authorization bypass in the Web, OpenAI, platform, ACP, or
  channel surfaces.
- Cross-profile credential, session, memory, artifact, or external-account
  access.
- Approval forgery, replay, confused-deputy behavior, or mutation without the
  required confirmation.
- Credential disclosure through logs, diagnostics, process arguments, browser
  responses, plugin state, or release artifacts.
- Sandbox escape, path traversal, unsafe symlink handling, arbitrary file access,
  or command execution beyond granted authority.
- SSRF or untrusted endpoint selection that reaches a protected network boundary.
- Plugin capability bypass, unsafe host imports, or cross-plugin state access.
- Channel webhook-verification bypass, account confusion, or delivery partition
  escape.
- Signed-release, update, rollback, backup, restore, or promotion verification
  bypass.
- Resource-limit bypass that enables unauthenticated or persistent denial of
  service.

## Usually not a security vulnerability

These reports generally need an additional boundary bypass:

- Prompt injection or undesirable model output by itself.
- A trusted operator intentionally approving a command or using an advertised
  local execution feature.
- A plugin, skill, or MCP server acting within authority the operator knowingly
  granted it.
- Public exposure caused solely by ignoring the documented authentication, TLS,
  origin, or network-policy requirements.
- Third-party provider outages, billing disputes, or vulnerabilities with no
  demonstrated Keith-specific impact.
- Scanner output, dependency reachability, or stale-code findings without a
  reproducible path and demonstrated impact in a shipped or current build.
- Test fixtures or maintainer-only fault injectors that are not reachable in the
  supported runtime.
- Availability costs that remain inside authenticated, configured resource
  limits and do not produce a persistent failure or boundary escape.

If you are unsure, report privately. Maintainers would rather route a careful
report than miss a real vulnerability.

## Deployment guidance

Before exposing Keith outside a trusted host:

1. Use a tagged image or immutable digest and verify its provenance.
2. Put every HTTP surface behind TLS and a restrictive ingress policy.
3. Generate different high-entropy values for every Keith and provider secret.
4. Keep `/var/lib/keith`, backups, signing keys, and credential master keys out
   of shared or publicly readable storage.
5. Run one deployment per mutually untrusted host-administration boundary.
6. Review enabled services, tools, plugins, skills, MCP servers, channel senders,
   and connected apps.
7. Back up durable state and prove restore and rollback before upgrading.

See the [deployment guide](docs/deployment.md) for provider-specific controls.

## Research guidelines and safe harbor

We support good-faith research intended to improve Keith's security. Stay within
your own accounts and infrastructure, minimize data access and service
disruption, stop when you encounter other people's data, and report the issue
privately. Do not use social engineering, denial of service, credential theft,
privacy invasion, or destructive testing.

## Security validation

Security-sensitive changes must preserve Keith's independent authentication,
authority, approval, credential, sandbox, evaluator, and promotion boundaries.
The repository CI combines dependency audits and static checks with real process,
browser, protocol, packaged-release, and container journeys. No single scanner or
unit test is treated as sufficient proof.

For local validation, start with:

```bash
cargo security-gate
cargo deny check
cargo audit
```

Run the focused live or packaged journey for every boundary the change affects,
and describe both completed and blocked validation honestly in the pull request.
