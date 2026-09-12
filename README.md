<h1 align="center">Keith</h1>

<p align="center">
  <strong>The first agent that truly evolves not through prompt tricks, but by modifying, testing, and safely promoting changes to its own harness.</strong>
</p>

<p align="center">
  Keith turns real experience into changes to the machinery that shapes how it
  reasons, chooses tools, manages context, and gets work done not another note
  in a prompt. Each new version is built in isolation, tested against the current
  Keith, and adopted only when it proves better without crossing your safety limits.
</p>

<p align="center">
  <a href="https://github.com/Sidiora-Labs/keith-agent-harness/actions/workflows/ci.yml"><img src="https://github.com/Sidiora-Labs/keith-agent-harness/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent-harness/releases/latest"><img src="https://img.shields.io/github/v/release/Sidiora-Labs/keith-agent-harness?display_name=tag" alt="Latest release"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent-harness/stargazers"><img src="https://img.shields.io/github/stars/Sidiora-Labs/keith-agent-harness?style=flat" alt="GitHub stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="License: Apache-2.0"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent-harness/pkgs/container/keith-agent"><img src="https://img.shields.io/badge/container-GHCR-blue" alt="GHCR image"></a>
</p>

<p align="center">
  <a href="docs/installation.md">Get started</a> ·
  <a href="docs/deployment.md">Deploy</a> ·
  <a href="docs/crate-guide.md">Architecture</a> ·
  <a href="CONTRIBUTING.md">Contribute</a> ·
  <a href="SECURITY.md">Security</a>
</p>

<p align="center">
  <img src="docs/assets/keith-harness-repair.png" alt="Keith testing a repair to its own harness in isolation" width="1100">
</p>

<p align="center"><sub>Learn from real work. Build a better harness. Prove the improvement before it goes live.</sub></p>

> [!IMPORTANT]
> Keith is pre-release software. The core system is working, but interfaces,
> storage, and packaging can still change before 1.0. Keith is part of Sidiora's Experimental Harness development program

## Try it

The shortest path is Docker:

```bash
cp .env.example .env
# Set KEITH_WEB_LOGIN_SECRET and one model-provider key in .env.
docker compose up --build
```

Open <http://localhost:7341>. Keith keeps its state in the `keith-data` volume
and can work inside the current checkout at `/workspace`.

Developing from source instead?

```bash
./keith doctor
./keith setup
./keith dev
```

See the [installation guide](docs/installation.md) for the TUI, provider setup,
upgrades, backups, and service management.

## How Keith evolves

Every run gives Keith more than a transcript. It produces evidence about how
well the whole agent worked: the reasoning path, tool choices, context use,
latency, cost, recovery, and final result.

A hard failure can expose an opportunity to evolve, but so can a wasteful tool
call, a slow recovery, or a result that should have been better.

Keith turns the strongest opportunity into a testable hypothesis, builds several
candidate harnesses away from the live system, and compares them with the current
version on work the proposer was not allowed to see. A candidate advances only
when it produces a measurable improvement without introducing a regression.

```text
experience → opportunity → testable hypothesis → candidate harnesses
           → held-out evaluation → canary → observe → keep or reverse
```

That is the central bet behind Keith: an agent should not only do work. It
should have a safe, inspectable way to become better at doing the work.

### Correct it while it works

Keith Computer is a visible, isolated desktop. Watch the run live, take the
keyboard and pointer in one action, correct the problem, then hand control back.
An exclusive control lease prevents you and Keith from fighting over the same
screen.

### Teach the work, not another prompt

When you demonstrate a task, Keith records the useful structure behind it:
screen state, keyboard and pointer actions, UI targets, application context,
timing, narration, files, clipboard activity, and every control handoff. It turns
that evidence into an editable **TaskRecipe** with inputs, checkpoints,
approvals, recovery steps, versions, and rollback.

You are not merely giving Keith a screen recording. You are showing it a piece
of work that it can replay and improve.

### Let it repair the harness not the rules

Self-repair is only useful if the candidate cannot move the goalposts. Keith's
repair candidates cannot edit the judge, held-back test cases, credentials,
what Keith may remember or reveal, your approval rules, release checks, the
promotion gate, or the rollback path.

Choose how far Keith may go:

- **Advise only**   explain the proposed repair and wait.
- **Shadow test**   build and test candidates, but do not promote one.
- **Autonomous repair**   canary, observe, and reverse within limits you set.

Every mode keeps the protected rules outside the repair candidate's reach.

### One Keith, not a folder of bots

The Web UI, TUI, OpenAI-compatible API, native API, ACP clients, messaging
channels, connected apps, and computer all reach the same daemon-owned agent.
Specialist workers can research, code, or operate tools behind the scenes without
turning the product into a dashboard full of personalities you have to manage.

Sessions survive client disconnects. Commitments, waits, scheduled work, goals,
and active runs can recover after a restart. Start in the terminal, check in from
Slack, and finish in the browser without creating three unrelated assistants.

## What you can do with Keith

| You want to… | Keith can… |
| --- | --- |
| Hand off a browser or desktop task | Work in a headed or headless computer while you watch, pause, or take over |
| Teach a repeatable workflow | Turn a live demonstration into an editable, replayable TaskRecipe |
| Stop repeating the same agent failure | Diagnose the harness, test competing repairs, canary the winner, and roll back |
| Use your own models | Route profiles through OpenAI, Anthropic, OpenRouter, Ollama, or another supported provider |
| Reach the same agent from anywhere | Serve Web, TUI, ACP, API, Discord, Slack, Telegram, WhatsApp, Teams, Google Chat, email, and Matrix clients |
| Connect real services | Use approval-gated connected apps, Composio, MCP servers, and capability-scoped WASI plugins |
| Build on top of Keith | Use the OpenAI-compatible `/v1` API or the typed native `/platform/v1` API |
| Run it on your infrastructure | Deploy with Docker, Kubernetes, Railway, Fly.io, DigitalOcean, Azure, AWS, or Google Cloud |

## One runtime, many ways in

```text
Web · TUI · OpenAI API · Platform API · ACP · Channels
                         │
                      agentd
              sessions · policy · recovery
                         │
                 leased agent workers
                         │
       models · tools · plugins · CUA · connected apps
```

`agentd` owns the truth. Clients render its sessions and lifecycle instead of
inventing their own state. Workers execute turns under leases, and domain crates
keep policy separate from external adapters.

Read the [crate guide](docs/crate-guide.md) and
[dependency boundaries](docs/architecture/dependency-boundaries.md) for the full
system map.

## APIs and extensions

Keith exposes two HTTP surfaces:

- **OpenAI-compatible `/v1`** for existing SDKs and tools such as Open WebUI.
- **Native `/platform/v1`** for trusted clients that need Keith's sessions,
  lifecycle, approvals, artifacts, and live events.

Extensions can run as capability-scoped WASI components, MCP servers, skills,
or approval-gated connected apps. ACP clients can attach through the bundled
stdio server. See [OpenAI compatibility](docs/openai-compatibility.md) and
[platform integration](docs/platform-integration.md).

## Security

> [!WARNING]
> Keith can run commands, change files, control a browser, and call external
> services with the authority you grant it. Use a workspace you can inspect and
> restore. Treat model output, channel messages, fetched pages, plugins, skills,
> MCP servers, and repair candidates as untrusted.

Keep the Web UI and APIs on loopback unless you add TLS, strong authentication,
and an explicit network policy. Use different secrets for Web login, APIs, model
providers, and release signing. Never post credentials or unredacted traces in
a public issue.

Report vulnerabilities privately through GitHub's
[security advisory form](https://github.com/Sidiora-Labs/keith-agent-harness/security/advisories/new).
Read [SECURITY.md](SECURITY.md) for the trust model, scope, and reporting rules.

## Deploy

Keith ships as one stateful OCI image, with supported paths for Docker Compose,
Kubernetes with Helm, Railway, Fly.io, DigitalOcean Kubernetes, Azure Kubernetes
Service, Amazon EKS, and Google Kubernetes Engine Autopilot.

```bash
./keith deploy kubernetes --render
./keith deploy railway
./keith deploy fly --app my-keith
./keith deploy aws --cluster keith --region us-east-1
```

Cloud commands print a plan by default. A deployment changes infrastructure only
when you pass `--execute` and set `KEITH_DEPLOY_APPROVED=YES`. Read the full
[deployment guide](docs/deployment.md) before exposing Keith outside the host.

## Develop and extend

The `./keith` command is the contributor entry point for setup, local services,
checks, tests, release builds, container images, scaffolding, and deployment
plans. Rust build output stays outside the checkout.

```bash
./keith check
./keith test
./keith image keith-agent:dev
./keith scaffold plugin my-plugin
./keith scaffold skill my-skill
```

Rust 1.93, Node.js 22.22, Corepack, and Git are required. Before opening a pull
request, read [CONTRIBUTING.md](CONTRIBUTING.md) and report the commands and real
user paths you actually exercised.

## Documentation

| Goal | Start here |
| --- | --- |
| Install, configure a provider, or run the TUI | [Installation and lifecycle](docs/installation.md) |
| Run with Docker or a cloud provider | [Deployment guide](docs/deployment.md) |
| Connect an OpenAI SDK or Open WebUI | [OpenAI compatibility](docs/openai-compatibility.md) |
| Integrate a trusted native client | [Platform integration](docs/platform-integration.md) |
| Understand the workspace | [Crate guide](docs/crate-guide.md) |
| Qualify a release | [Release qualification](docs/release-qualification.md) |
| Ask for help or report a problem | [Support](SUPPORT.md) |

## Community

- Ask questions and share ideas in
  [GitHub Discussions](https://github.com/Sidiora-Labs/keith-agent-harness/discussions).
- Report reproducible bugs through the
  [issue forms](https://github.com/Sidiora-Labs/keith-agent-harness/issues/new/choose).
- Report security problems privately, never in a public issue.
- Follow the [Code of Conduct](CODE_OF_CONDUCT.md) in every project space.

## License

Keith is available under the [Apache License 2.0](LICENSE).
