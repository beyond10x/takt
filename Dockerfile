ARG RUST_IMAGE=docker.io/library/rust:1.99-slim-trixie

FROM ${RUST_IMAGE} AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends git pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p b10x-takt-cli

FROM ${RUST_IMAGE} AS tools
ARG AEP_VERSION=0.77.1
ARG AEP_SHA256=24121f808c52b5c76904ba03516229f748e3a5ab7a195552ad7d45c90d0714b2
ARG HARNESS_VERSION=0.18.0
ARG HARNESS_SHA256=ebfc1e40f5f0cfe0d5e0a5d2155975847487123007ddaf688010539d5b5bb9be
ARG ESS_VERSION=0.69.2
ARG ESS_SHA256=8cc5a3a7ed7c27aacbff131ab1437e18e7d7d1878fee77542b3e0fff7157d414
ADD --checksum=sha256:${AEP_SHA256} https://github.com/beyond10x/aep/releases/download/${AEP_VERSION}/aep-${AEP_VERSION}-x86_64-unknown-linux-gnu.tar.gz /dl/aep.tar.gz
ADD --checksum=sha256:${HARNESS_SHA256} https://github.com/beyond10x/harness/releases/download/${HARNESS_VERSION}/b10x-harness-${HARNESS_VERSION}-x86_64-unknown-linux-gnu.tar.gz /dl/harness.tar.gz
ADD --checksum=sha256:${ESS_SHA256} https://github.com/beyond10x/ess/releases/download/${ESS_VERSION}/ess-${ESS_VERSION}-x86_64-unknown-linux-gnu.tar.gz /dl/ess.tar.gz
RUN mkdir -p /out \
 && tar -xzf /dl/aep.tar.gz -C /out --strip-components=1 "aep-${AEP_VERSION}-x86_64-unknown-linux-gnu/aep" \
 && tar -xzf /dl/harness.tar.gz -C /out --strip-components=1 "b10x-harness-${HARNESS_VERSION}-x86_64-unknown-linux-gnu/b10x-harness" \
 && tar -xzf /dl/ess.tar.gz -C /out --strip-components=1 "ess-${ESS_VERSION}-x86_64-unknown-linux-gnu/ess"

FROM ${RUST_IMAGE}
ARG REVISION=unknown
LABEL org.opencontainers.image.source="https://github.com/beyond10x/takt" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.licenses="Apache-2.0"
RUN apt-get update \
 && apt-get install -y --no-install-recommends git libssl3t64 python3 \
 && rm -rf /var/lib/apt/lists/* \
 && rustup component add clippy rustfmt \
 && useradd --uid 10001 --user-group --create-home --home-dir /var/lib/takt takt \
 && mkdir -p /workspace /config \
 && chown takt:takt /workspace
COPY --from=tools /out/ /usr/local/bin/
COPY --from=build /src/target/release/takt /usr/local/bin/takt
USER 10001:10001
ENV HOME=/var/lib/takt
WORKDIR /workspace
ENTRYPOINT ["takt", "run", "--repo", "/workspace", "--config", "/config/run.toml"]
