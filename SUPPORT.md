# Support

Keith runs across your machine, model provider, workspace, browser, and optional
external services. Choose the channel that matches the problem and include the
environment details needed to reproduce it.

| You have | Go to |
| --- | --- |
| A setup, configuration, or usage question | [GitHub Discussions](https://github.com/machinecity/keith-agent/discussions) |
| An idea or architecture proposal | [GitHub Discussions](https://github.com/machinecity/keith-agent/discussions) |
| Reproducible behavior that should change | [Bug report](https://github.com/machinecity/keith-agent/issues/new/choose) |
| A focused feature request | [Feature request](https://github.com/machinecity/keith-agent/issues/new/choose) |
| A vulnerability or leaked credential | [Private security advisory](https://github.com/machinecity/keith-agent/security/advisories/new) |
| A contribution question | [Contributing guide](CONTRIBUTING.md) |

Security vulnerabilities never belong in a public thread. Follow
[SECURITY.md](SECURITY.md), even if you are unsure whether the finding is
exploitable.

## What to include

Most reports are resolved faster when they include:

- Keith version or full commit SHA.
- Operating system and architecture.
- Installation method: source, signed archive, Docker, or cloud provider.
- The affected surface: Web, TUI, daemon, OpenAI API, ACP, channel, plugin,
  connected app, computer use, deployment, or build tooling.
- Model provider and model name, without the credential.
- Minimal reproduction steps and expected versus observed behavior.
- Sanitized logs and screenshots when useful.
- Whether the behavior survives restart or occurs in a fresh data root.
- The output of `./keith doctor` for development-tooling problems.

For deployment problems, also include the provider, region, image tag or digest,
health status, and redacted service logs. State whether you are reporting local
proof, a deployed health check, or a complete authenticated user journey; those
are different levels of evidence.

## Protect your data

Public GitHub content is indexed and may remain available after editing. Before
posting, remove provider keys, OAuth tokens, Web and API secrets, signing keys,
cookies, credential files, private prompts, personal data, internal hostnames,
and unredacted execution traces.

Use synthetic examples whenever possible. If you accidentally disclose a
credential, revoke or rotate it immediately; editing the post is not sufficient.

## What to expect

Maintainers and community members triage reproducible reports as time permits.
A question may be redirected to documentation, an integration, or a plugin when
the behavior does not belong in the core runtime. A bug may require a minimal
reproduction or current-build confirmation before it can be investigated.

Keith is pre-release software. Support is best effort, and there is no guaranteed
response time. Clear, focused reports with safe evidence are the fastest route to
an actionable answer.
