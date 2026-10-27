---
title: Install
sidebar_position: 1
description: Install takt 0.14.0 from a release asset with its checksum, with cargo from the tag, or as a container image you build, and the tools each kind of run needs beside it.
lede: takt is one binary; what else you install depends on the harness, the forge and whether you deliver stories.
source: the release assets of 0.14.0 (checksum commands run); cargo install --git at the tag 0.14.0 (run, 3 min 27 s); docker build of the Dockerfile at 0.14.0 (run); takt run --help at 0.14.0; Dockerfile
---

# Install

Three ways to get takt 0.14.0. They give the same `takt` binary.

| way | needs | gets you |
|---|---|---|
| release asset | `curl`, `sha256sum` or `shasum` | a binary for Linux x86_64, Linux aarch64 or macOS on Apple silicon |
| `cargo install` | a Rust toolchain | a binary built from the tag on your machine |
| container image | Docker or another OCI builder | an x86_64 image with takt and its tools, built by you |

## From a release asset

```sh
version=0.14.0 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
```

The targets are `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and
`aarch64-apple-darwin`. Releases 0.7.0 to 0.9.0 have no macOS binary, and 0.13.0 has no release
at all: its macOS build failed, and 0.13.1 ships its changes. On macOS, where `sha256sum` may be
missing:

```sh
grep " takt-$version-$target.tar.gz\$" SHA256SUMS | shasum -a 256 --check
```

The archive holds `takt`, `README.md`, `CHANGELOG.md` and `LICENSE` in the directory
`takt-<version>-<target>/`. Move `takt` onto your `PATH`.

## With cargo

From the tag, without a checkout:

```sh
cargo install --locked --git https://github.com/beyond10x/takt --tag 0.14.0 b10x-takt-cli
```

The package is `b10x-takt-cli`, and the binary it installs is `takt`. A build took 3 minutes 27
seconds here. cargo warns about a yanked `tokio` in the lock file; the build is `--locked` and
goes on. From a checkout of the repository:

```sh
cargo install --locked --path crates/takt-cli
```

```console
$ takt --version
takt 0.14.0 (da165bb1383b30de345842c6e4c508e99bf38c4d)
```

## As a container image

No takt image is published. Build one from a checkout of the tag:

```sh
git clone --branch 0.14.0 https://github.com/beyond10x/takt
cd takt
docker build --build-arg REVISION=$(git rev-parse HEAD) -t takt:0.14.0 .
```

The image is about 1.3 GB. It holds takt, `aep`, `b10x-harness`, `ess`, `b10x-gates` and a Rust
toolchain, and runs as uid 10001. Without `REVISION`, the image's label names an `unknown`
revision. What is in it and how it is mounted is in
[Container image](../reference/container-image.md), and running it in
[Run in a container](./run-in-a-container.md).

## The tools beside takt

takt starts other programs and finds them on the `PATH`. Which ones depends on the run:

| run | tools |
|---|---|
| every run | `git`, and the test command's toolchain (`cargo` by default) |
| `--harness claude`, the default | `claude` (Claude Code), logged in |
| `--harness codex` | `codex`, and `b10x-gates` with `B10X_GATES_POLICY` set |
| `--harness harness` | `b10x-harness` 0.20.0, an LLM endpoint, and `b10x-gates` with `B10X_GATES_POLICY` set |
| a story | `aep` and a planning store; a gates document for the gate |
| `--forge bot` | `b10x-gates` and `gh` |
| `--forge gitlab` | `glab`, with `glab auth status --hostname <host>` exiting 0 |
| `takt promote` | `bwrap`, and rustup's default toolchain |

The neighbours, each with its own docs:
[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
keeps the planning store,
[b10x-harness](https://beyond10x.github.io/harness/) ([GitHub](https://github.com/beyond10x/harness))
is the agent that talks to an LLM endpoint, and
[metaharness](https://beyond10x.github.io/metaharness/) ([GitHub](https://github.com/beyond10x/metaharness))
is the layer every harness runs through. metaharness is a library inside takt, not a program to
install.

On Linux, the test command always runs in a user and mount namespace of its own, and there is no
way to turn that off. A host or container that refuses unprivileged user namespaces ends the
first test run on an error, exit 5. In a container, that means running it with seccomp and
AppArmor unconfined ([Run in a container](./run-in-a-container.md)).
