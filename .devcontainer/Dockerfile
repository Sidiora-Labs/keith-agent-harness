FROM node:22.22.2-bookworm AS node

FROM rust:1.93.0-bookworm

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
       bash ca-certificates clang cmake curl git jq libssl-dev pkg-config ripgrep \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/ /usr/local/

WORKDIR /workspaces/keith-agent
