# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.93.0
ARG NODE_VERSION=22.22.2

FROM node:${NODE_VERSION}-bookworm-slim AS web
WORKDIR /src/apps/agent-web/ui
RUN corepack enable \
    && corepack prepare pnpm@11.18.0 --activate
COPY apps/agent-web/ui/package.json apps/agent-web/ui/pnpm-lock.yaml apps/agent-web/ui/pnpm-workspace.yaml ./
RUN --mount=type=cache,id=keith-pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY apps/agent-web/ui/ ./
RUN pnpm run check && pnpm run test && pnpm run build

FROM rust:${RUST_VERSION}-bookworm AS builder
RUN apt-get update \
    && apt-get install --yes --no-install-recommends clang cmake libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
COPY --from=web /src/apps/agent-web/static/ui/ apps/agent-web/static/ui/
ARG KEITH_BUILD_ID=container
ENV KEITH_BUILD_ID=${KEITH_BUILD_ID} \
    CARGO_INCREMENTAL=0
RUN --mount=type=cache,id=keith-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=keith-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=keith-cargo-target,target=/src/target \
    cargo build --workspace --release --locked \
      --bin agentd \
      --bin agent-worker \
      --bin agent-cli \
      --bin agent-tui \
      --bin agent-web \
      --bin channel-gateway \
      --bin tool-runner \
      --bin browser-runner \
      --bin kernel-runner \
      --bin keith-agent-acp \
      --bin keith-composio-mcp \
      --bin keith-cua-runner \
      --bin keith-performance-runner \
    && mkdir -p /out/bin /out/web /out/providers \
    && for binary in \
        agentd agent-worker agent-cli agent-tui agent-web channel-gateway \
        tool-runner browser-runner kernel-runner keith-agent-acp \
        keith-composio-mcp keith-cua-runner keith-performance-runner; do \
         install -m 0755 "target/release/${binary}" "/out/bin/${binary}"; \
       done \
    && cp -a apps/agent-web/static/ui /out/web/ui \
    && cp packaging/providers.json /out/providers/providers.json

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
       bash bubblewrap ca-certificates chromium curl git jq openssh-client \
       python3 ripgrep tini util-linux xvfb \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 keith \
    && useradd --uid 10001 --gid keith --create-home --shell /bin/bash keith \
    && mkdir -p /opt/keith /var/lib/keith /workspace \
    && chown -R keith:keith /var/lib/keith /workspace
COPY --from=builder /out/bin/ /usr/local/bin/
COPY --from=builder /out/web/ /opt/keith/web/
COPY --from=builder /out/providers/ /opt/keith/providers/
COPY --chmod=0755 packaging/container/entrypoint.sh /usr/local/bin/keith-entrypoint
COPY --chmod=0755 packaging/container/healthcheck.sh /usr/local/bin/keith-healthcheck

ENV KEITH_DATA_ROOT=/var/lib/keith \
    KEITH_WORKSPACE_ROOT=/workspace \
    KEITH_ASSET_ROOT=/opt/keith/web \
    KEITH_SERVICES=channels,acp,plugins,connected_apps,computers,teaching \
    PORT=7341
EXPOSE 7341
USER keith
WORKDIR /workspace
HEALTHCHECK --interval=15s --timeout=5s --start-period=45s --retries=4 CMD ["keith-healthcheck"]
ENTRYPOINT ["/usr/bin/tini", "--", "keith-entrypoint"]
