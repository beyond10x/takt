---
title: Install
sidebar_position: 1
description: Install takt 0.18.1 from a release asset with its checksum, with cargo from the tag, or as a container image you build, and the tools each kind of run needs beside it.
lede: takt is one binary; what else you install depends on the harness, the forge and whether you deliver stories.
source: the release assets of 0.14.0 (checksum commands run); cargo install --git at the tag 0.14.0 (run, 3 min 27 s); docker build of the Dockerfile at 0.14.0 (run); takt run --help at 0.14.0; Dockerfile
---

# Install

Three ways to get takt 0.18.1. They give the same `takt` binary.

| way | needs | gets you |
|---|---|---|
| release asset | `curl`, `sha256sum` or `shasum` | a binary for Linux x86_64, Linux aarch64 or macOS on Apple silicon |
| `cargo install` | a Rust toolchain | a binary built from the tag on your machine |
| container image | Docker or another OCI builder | an x86_64 image with takt and its tools, built by you |

## From a release asset

```sh
version=0.18.1 target=x86_64-unknown-linux-gnu
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
cargo install --locked --git https://github.com/beyond10x/takt --tag 0.18.1 b10x-takt-cli
```

The package is `b10x-takt-cli`, and the binary it installs is `takt`. A build took 3 minutes 27
seconds here. cargo warns about a yanked `tokio` in the lock file; the build is `--locked` and
goes on. From a checkout of the repository:

```sh
cargo install --locked --path crates/takt-cli
```

```console
$ takt --version
takt 0.18.1 (<the commit the 0.18.1 tag names>)
```

## As a container image

No takt image is published. Build one from a checkout of the tag:

```sh
git clone --branch 0.18.1 https://github.com/beyond10x/takt
cd takt
docker build --build-arg REVISION=$(git rev-parse HEAD) -t takt:0.18.1 .
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
| `--harness codex` | `codex`, and a [scan adapter](../reference/configuration.md#the-scan): `b10x-gates` with `B10X_GATES_POLICY` set for the default `gates`, or `scan: none` declared in the run's file |
| `--harness harness` | `b10x-harness` 0.20.0, an LLM endpoint, and a scan adapter, as for `codex` |
| a story | `aep` and a planning store; a gates document for the gate |
| `--forge bot` (beyond10x's route, github.com only) | `b10x-gates` acting as the bot's GitHub App, and `gh` |
| `--forge gitlab` | `glab`, with `glab auth status --hostname <host>` exiting 0 |
| `takt promote` | `bwrap` on Linux, `/usr/bin/sandbox-exec` on macOS; and rustup's default toolchain |

A gate's commands are the exception: they find a program by name on `/usr/local/bin:/usr/bin:/bin`,
or on the `PATH` their `env` sets, as the [run-a-story guide](./run-a-story.md) states.

The neighbours, each with its own docs:
[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
keeps the planning store,
[b10x-harness](https://beyond10x.github.io/harness/) ([GitHub](https://github.com/beyond10x/harness))
is the agent that talks to an LLM endpoint, and
[metaharness](https://beyond10x.github.io/metaharness/) ([GitHub](https://github.com/beyond10x/metaharness))
is the layer every harness runs through. metaharness is a library inside takt, not a program to
install.

On Linux and macOS the test command runs through Substrate's confinement contract. Under the
default `--confinement none` it starts through Substrate's `local` adapter, declared insecure: on
the host as this user, in no namespace of takt's own. `--confinement substrate` runs it through the
first of Substrate's `systemd` around `linux`, `linux` and `macos` adapters that serves it, and
refuses it by name where none does ([Getting started](../getting-started.md)). Up to 0.14.0, `none`
ran it in a user and mount namespace of its own, which a host or container that refuses
unprivileged user namespaces ended on an error, exit 5; in a container that meant running it with
seccomp and AppArmor unconfined ([Run in a container](./run-in-a-container.md)), which has not been
measured again since. On macOS, up to 0.14.0, `none` also ran it under takt's Seatbelt profile,
which limited its writes, the programs it started, launchd and the keychain; Substrate's `local`
adapter limits none of them.
