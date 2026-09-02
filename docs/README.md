# Keith documentation

This is the map for Keith's public documentation. Start with the path that
matches what you are trying to do, then use the feature guides for behavior,
boundaries, limitations, and source-level detail.

## Start here

| Goal | Guide |
| --- | --- |
| Run Keith for the first time | [Installation and lifecycle](installation.md) |
| Choose and configure a model provider | [Providers and model routing](features/providers-and-model-routing.md) |
| Use the browser interface | [Web interface](features/web-interface.md) |
| Use Keith from a terminal | [Terminal interface](features/terminal-interface.md) |
| Deploy with Docker or a cloud provider | [Deployment](deployment.md) |
| Connect an existing OpenAI client | [OpenAI-compatible API](features/openai-compatible-api.md) |
| Build a native integration | [Native platform API](features/native-platform-api.md) |
| Understand Keith's security model | [Security policy](../SECURITY.md) |
| Contribute code or documentation | [Contributing](../CONTRIBUTING.md) |
| Troubleshoot or ask for help | [Support](../SUPPORT.md) |

## How capability status is described

Keith is pre-release. The documentation deliberately separates these levels of
evidence:

1. **Implemented** means the production path exists in source.
2. **Locally qualified** means a focused test exercised the real Keith types,
   process, transport, browser, or component boundary on local infrastructure.
3. **Externally qualified** means an authorized real provider, channel,
   connected account, or cloud service completed the documented journey.
4. **Release-qualified** means the exact packaged artifacts passed the complete
   cross-platform, security, restart, migration, performance, and soak matrix.

Source presence is not proof of external or release qualification. Feature
guides list current limitations and blocked journeys instead of presenting them
as successes.

## Core agent

| Feature | Guide |
| --- | --- |
| Daemon, workers, conversations, branches, events, and recovery | [Runtime and sessions](features/runtime-and-sessions.md) |
| Provider catalog, credentials, model discovery, and routing | [Providers and model routing](features/providers-and-model-routing.md) |
| Durable memory, evidence, relationships, recall, and retrieval | [Memory and retrieval](features/memory-and-retrieval.md) |
| Goals, plans, review, initiative, and continuation | [Goals, planning, and initiative](features/goals-planning-and-initiative.md) |
| Tools, approvals, workspaces, artifacts, and sandboxing | [Tools, workspaces, and artifacts](features/tools-workspaces-and-artifacts.md) |
| Harness diagnosis, candidate evaluation, promotion, and reversal | [Self-evolution](features/self-evolution.md) |

## Interfaces and APIs

| Feature | Guide |
| --- | --- |
| Browser client | [Web interface](features/web-interface.md) |
| Ratatui terminal client | [Terminal interface](features/terminal-interface.md) |
| Local desktop lifecycle and signed updates | [Desktop interface](features/desktop-interface.md) |
| OpenAI Chat Completions compatibility | [OpenAI-compatible API](features/openai-compatible-api.md) |
| Typed native HTTP commands and event streams | [Native platform API](features/native-platform-api.md) |
| ACP v1 and the isolated draft-v2 boundary | [Agent Client Protocol](features/agent-client-protocol.md) |

## Computer use and durable autonomy

| Feature | Guide |
| --- | --- |
| Isolated visual computer and exclusive human/agent control | [Computer use](features/computer-use.md) |
| Demonstration capture, editable recipes, replay, and publication | [Teaching and TaskRecipes](features/teaching-and-task-recipes.md) |
| Schedules, waits, commitments, awareness, attention, and delivery | [Automation and proactive work](features/automation-and-proactive-work.md) |
| Delegated child sessions and memory scouts | [Child agents](features/child-agents.md) |

## Extensions and external systems

| Feature | Guide |
| --- | --- |
| Discord, Slack, Telegram, WhatsApp Cloud, Teams, Google Chat, email, and Matrix | [Messaging channels](features/messaging-channels.md) |
| Building another channel adapter | [Channel adapter development](features/channel-adapter-development.md) |
| Profile-scoped connected accounts and Composio | [Connected apps and Composio](features/connected-apps-and-composio.md) |
| Managed stdio and HTTP tool servers | [MCP servers](features/mcp-servers.md) |
| Capability-scoped WebAssembly components | [Plugins](features/plugins.md) |
| Declarative procedures and learned skills | [Skills](features/skills.md) |

## Operation and data

| Feature | Guide |
| --- | --- |
| Service configuration, profiles, credentials, export, restore, and deletion | [Configuration, profiles, and data](features/configuration-profiles-and-data.md) |
| Diagnostics, telemetry, limits, admission, and reclamation | [Observability and resource governance](features/observability-and-resource-governance.md) |
| Latency, resource measurements, concurrency, and soak tests | [Performance and soak](features/performance-and-soak.md) |
| Docker, Kubernetes, Railway, Fly.io, DigitalOcean, Azure, AWS, and Google Cloud | [Deployment](deployment.md) |
| Backups, signed updates, rollback, and uninstall | [Installation and lifecycle](installation.md) |
| Artifact-wide acceptance matrix | [Release qualification](release-qualification.md) |

## Architecture and contributor references

| Topic | Guide |
| --- | --- |
| All workspace packages and ownership | [Crate guide](crate-guide.md) |
| Allowed internal dependency direction | [Dependency boundaries](architecture/dependency-boundaries.md) |
| Contributor setup, test integrity, and pull requests | [Contributing](../CONTRIBUTING.md) |
| Repository instructions for coding agents | [Agent guide](../AGENTS.md) |
| Security reporting and trust boundaries | [Security policy](../SECURITY.md) |
| Community and troubleshooting routes | [Support](../SUPPORT.md) |

## Protocol and format references

- [AgentConnection schema](reference/agent-connection.md) is generated from the
  current public client/daemon protocol.
- [Common types schema](reference/common-types.md) is generated from the shared
  versioned types.
- [Data-control format](../crates/data-control/FORMAT.md) defines the standalone
  export and restore representation.
- [Discord setup](discord.md) contains the currently documented direct gateway
  setup for the Discord executable path.
- [OpenAI compatibility legacy link](openai-compatibility.md) and
  [native platform legacy link](platform-integration.md) are retained for
  existing inbound links and point to their canonical feature guides.

Files marked with a generated banner must be updated through their source
generator. Public feature guides are maintained by contributors and should be
updated in the same change as observable behavior, configuration, or status.
