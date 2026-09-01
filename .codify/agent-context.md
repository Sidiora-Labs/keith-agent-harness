# agent-keith — Agent Context

<!-- codify-owned: graph-agent-context v1 -->

_Generated graph context owned by `cg agentmd`. Regenerate with `cg agentmd --write` after significant changes. Workflow instructions remain owned by `cg spec render`._

## Languages

| Language | Files | Lines |
|---|---:|---:|
| rust | 141 | 124987 |
| javascript | 18 | 646 |
| typescript | 8 | 2544 |

167 source files, 128177 lines total.

## Directory map

- `apps/` — 64 files, 20542 lines (mostly rust)
- `crates/` — 103 files, 107635 lines (mostly rust)

## Build & tooling

- `Cargo.toml` — Cargo (Rust)

## Entry points

- function `main` — `apps/agent-cli/src/main.rs:246`
- function `main` — `apps/agent-desktop/src/main.rs:15`
- function `main` — `apps/agent-desktop/tests/support/daemon_host.rs:11`
- function `main` — `apps/agent-desktop/tests/support/release_report_host.rs:5`
- function `main` — `apps/agent-tui/src/main.rs:186`
- function `main` — `apps/agent-web/src/main.rs:6`
- function `main` — `apps/agent-worker/src/main.rs:1`
- function `main` — `apps/agentd/src/main.rs:458`
- function `main` — `apps/agentd/tests/support/worker_process_host.rs:1`
- function `main` — `apps/browser-runner/src/main.rs:79`
- function `main` — `apps/channel-gateway/src/main.rs:771`
- function `main` — `apps/kernel-runner/src/main.rs:96`
- function `main` — `apps/performance-runner/src/main.rs:228`
- function `main` — `apps/tool-runner/src/main.rs:84`
- function `main` — `apps/xtask/src/main.rs:22`

## HTTP routes

| Method | Pattern | Handler | Where |
|---|---|---|---|
| GET | `/` | `app` | `apps/agent-web/src/server.rs:205` |
| GET | `/api/bootstrap` | `bootstrap` | `apps/agent-web/src/server.rs:209` |
| GET | `/api/events/{profile}/{session}` | `events` | `apps/agent-web/src/server.rs:217` |
| POST | `/api/evolution/commands` | `evolution_command` | `apps/agent-web/src/server.rs:210` |
| POST | `/api/profiles/{profile}/commands` | `command` | `apps/agent-web/src/server.rs:212` |
| GET | `/assets/ui/{*path}` | `ui_asset` | `apps/agent-web/src/server.rs:211` |
| POST | `/auth/session` | `create_session` | `apps/agent-web/src/server.rs:208` |
| GET | `/favicon.ico` | `favicon` | `apps/agent-web/src/server.rs:207` |
| GET | `/login` | `login` | `apps/agent-web/src/server.rs:206` |
| GET | `/platform/v1/catalog` | `platform_compat::catalog` | `apps/agent-web/src/server.rs:225` |
| GET | `/platform/v1/health` | `platform_compat::health` | `apps/agent-web/src/server.rs:224` |
| GET | `/v1/models` | `openai_compat::models` | `apps/agent-web/src/server.rs:218` |
| GET | `/v1/models/{model}` | `openai_compat::model` | `apps/agent-web/src/server.rs:219` |

## Load-bearing symbols (most referenced)

- `new` (function, 3231 refs) — `apps/agent-web/src/security.rs:59`
- `len` (function, 877 refs) — `crates/daemon-core/src/lib.rs:270`
- `path` (function, 652 refs) — `apps/agent-desktop/src/lib.rs:647`
- `is_empty` (function, 544 refs) — `crates/daemon-core/src/lib.rs:274`
- `from_unix_millis` (function, 532 refs) — `crates/agent-types/src/lib.rs:289`
- `push` (function, 496 refs) — `crates/memory/src/unified.rs:121`
- `default` (function, 485 refs) — `apps/agent-tui/src/lib.rs:77`
- `open` (function, 485 refs) — `apps/agent-desktop/src/lib.rs:590`
- `contains` (function, 455 refs) — `crates/scheduler/src/lib.rs:1066`
- `get` (function, 448 refs) — `crates/agent-types/src/lib.rs:245`
- `from` (function, 387 refs) — `crates/agent-types/src/lib.rs:180`
- `lock` (function, 285 refs) — `crates/action-store/src/lib.rs:673`
- `now` (function, 279 refs) — `crates/agent-types/src/lib.rs:300`
- `insert` (function, 237 refs) — `crates/evolution/src/refinement.rs:822`
- `write` (function, 232 refs) — `crates/evolution/src/refinement.rs:83`

## Querying this codebase

This project is indexed by Codify (SQLite + FTS5, 100% local). Prefer these over grep/file-walking — one call returns definitions, snippets, and call edges:

```bash
cg context <query>      # symbols + snippets + callers/callees + routes
cg search <text>        # instant name/full-text search
cg symbol <name>        # definition + snippet + reference count
cg impact <name> -d 3   # who breaks if this changes
cg routes [filter]      # URL pattern -> handler
cg changes              # impact radius of uncommitted edits
```

All of the above accept `--json`. The graph auto-syncs via `cg watch`, or connect over MCP with `cg mcp-install`.
