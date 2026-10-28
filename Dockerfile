ARG RUST_IMAGE=docker.io/library/rust:1.99-slim-trixie
ARG BASE_IMAGE=docker.io/library/debian:trixie-slim

FROM ${RUST_IMAGE} AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends git pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p b10x-takt-cli

FROM ${RUST_IMAGE} AS toolchain
RUN rustup component add clippy rustfmt \
 && mv "$(rustc --print sysroot)" /opt/rust \
 && chmod -R go-w /opt/rust

FROM ${RUST_IMAGE} AS tools
ARG AEP_VERSION=0.77.1
ARG AEP_SHA256=24121f808c52b5c76904ba03516229f748e3a5ab7a195552ad7d45c90d0714b2
ARG HARNESS_VERSION=0.20.0
ARG HARNESS_SHA256=782da29650857aa1b6c9a118f05a64304fb434af775b46e9cabbb4f541964a80
ARG ESS_VERSION=0.72.0
ARG ESS_SHA256=4fd09de042ec6220a3ed9ed9e07d2f7deb91f1fdb20c998340d5c8e224ec3f11
ARG GATES_VERSION=0.1.23
ARG GATES_SHA256=17299ab29c065908edc9d188667f0b954110c690ff9bcd6a370d04a3092cd035
ADD --checksum=sha256:${AEP_SHA256} https://github.com/beyond10x/aep/releases/download/${AEP_VERSION}/aep-${AEP_VERSION}-x86_64-unknown-linux-gnu.tar.gz /dl/aep.tar.gz
ADD --checksum=sha256:${HARNESS_SHA256} https://github.com/beyond10x/harness/releases/download/${HARNESS_VERSION}/b10x-harness-${HARNESS_VERSION}-x86_64-unknown-linux-gnu.tar.gz /dl/harness.tar.gz
ADD --checksum=sha256:${ESS_SHA256} https://github.com/beyond10x/ess/releases/download/${ESS_VERSION}/ess-${ESS_VERSION}-x86_64-unknown-linux-gnu.tar.gz /dl/ess.tar.gz
ADD --checksum=sha256:${GATES_SHA256} https://github.com/beyond10x/gates/releases/download/${GATES_VERSION}/b10x-gates-linux-x86_64 /dl/b10x-gates
RUN mkdir -p /out \
 && tar -xzf /dl/aep.tar.gz -C /out --strip-components=1 "aep-${AEP_VERSION}-x86_64-unknown-linux-gnu/aep" \
 && tar -xzf /dl/harness.tar.gz -C /out --strip-components=1 "b10x-harness-${HARNESS_VERSION}-x86_64-unknown-linux-gnu/b10x-harness" \
 && tar -xzf /dl/ess.tar.gz -C /out --strip-components=1 "ess-${ESS_VERSION}-x86_64-unknown-linux-gnu/ess" \
 && install -m 0755 /dl/b10x-gates /out/b10x-gates

FROM ${BASE_IMAGE}
ARG REVISION=unknown
LABEL org.opencontainers.image.source="https://github.com/beyond10x/takt" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.licenses="Apache-2.0"
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates git gcc libc6-dev pkg-config libssl-dev python3 tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --uid 10001 --user-group --create-home --home-dir /var/lib/takt takt \
 && mkdir -p /workspace /config \
 && chown takt:takt /workspace
COPY --from=toolchain /opt/rust /opt/rust
RUN for tool in /opt/rust/bin/*; do ln -s "$tool" "/usr/local/bin/${tool##*/}"; done
COPY --from=tools /out/ /usr/local/bin/
COPY --from=build /src/target/release/takt /usr/local/bin/takt
USER 10001:10001
ENV HOME=/var/lib/takt
WORKDIR /workspace
ENTRYPOINT ["tini", "--", "takt", "run", "--repo", "/workspace"]
CMD ["--config", "/config/agent.yaml"]
