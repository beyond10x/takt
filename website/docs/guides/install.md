---
title: Install
sidebar_position: 1
description: Install takt 0.26.0 from a release asset with its checksum, with cargo from the tag, or as a container image you build; the tools each kind of run needs beside it; and how the test command is confined.
lede: takt is one binary. What else you install depends on the harness, the forge and whether you deliver stories.
source: the release asset and checksum commands (run against 0.14.0's assets); install and command -v run on a build of 0.21.0's source; cargo install --git and docker build of the Dockerfile (run at 0.14.0); takt run --help of 0.21.0; the Dockerfile; metaharness 0.23.0 (PINNED_VERSIONS of metaharness-claude); runs of a build of 0.21.0's source with Claude Code 2.1.312 and 2.1.309, 2026-10-29
---

# Install

Three ways to get takt 0.26.0. They give the same `takt` binary.

| way | needs | gets you |
|---|---|---|
| release asset | `curl`, `sha256sum` or `shasum` | a binary for Linux x86_64, Linux aarch64 or macOS on Apple silicon |
| `cargo install` | a Rust toolchain | a binary built from the tag on your machine |
| container image | Docker or another OCI builder | an x86_64 image with takt and its tools, built by you |

## From a release asset

```sh
version=0.26.0 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
mkdir -p ~/.local/bin
install -m 755 "takt-$version-$target/takt" ~/.local/bin/takt
command -v takt
takt --version
```

The targets are `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and
`aarch64-apple-darwin`. If `command -v takt` prints nothing, `~/.local/bin` is not on your `PATH`;
add it in your shell's profile. `takt --version` prints the version and the commit it was built
from:

```console
$ takt --version
takt 0.26.0 (<the commit the 0.26.0 tag names>)
```

On macOS, where `sha256sum` may be missing, check the archive with:

```sh
grep " takt-$version-$target.tar.gz\$" SHA256SUMS | shasum -a 256 --check
```

The archive holds `takt`, `README.md`, `CHANGELOG.md` and `LICENSE` in the directory
`takt-<version>-<target>/`. Which release added what is in the
[changelog](https://github.com/beyond10x/takt/blob/main/CHANGELOG.md).

## With cargo

From the tag, without a checkout:

```sh
cargo install --locked --git https://github.com/beyond10x/takt --tag 0.26.0 b10x-takt-cli
```

The package is `b10x-takt-cli`; the binary it installs is `takt`, in `~/.cargo/bin`. From a
checkout of the repository:

```sh
cargo install --locked --path crates/takt-cli
```

## As a container image

No takt image is published. Build one from a checkout of the tag:

```sh
git clone --branch 0.26.0 https://github.com/beyond10x/takt
cd takt
docker build --build-arg REVISION=$(git rev-parse HEAD) -t takt:0.26.0 .
```

The image holds takt, `aep`, `b10x-harness`, `ess` and a Rust toolchain, and runs
as uid 10001. It holds no Claude Code. Without `REVISION`, the image's label names an `unknown`
revision. What is in it: [Container image](../reference/container-image.md). How to run it:
[Run in a container](./run-in-a-container.md).

## The tools beside takt

takt starts other programs and finds them on the `PATH`. Which ones depends on the run: the
[harness](../concepts/glossary.md#harness) that takes the model steps, the
[forge](../concepts/glossary.md#forge) a change leaves through, and whether you deliver a
[story](../concepts/glossary.md#story).

| run | tools |
|---|---|
| every run | [`git`](https://git-scm.com/downloads), and the test command's toolchain: `cargo test --quiet` by default ([rustup](https://rustup.rs)), any other with `--test-cmd` |
| `--harness claude`, the default | [Claude Code](https://github.com/anthropics/claude-code) (`claude`), logged in, at a [qualified version](#claude-codes-version) |
| `--harness codex` | [Codex](https://github.com/openai/codex) (`codex`), and `scan = "none"` declared in the run file ([Configure a run](./configure-a-run.md)) |
| `--harness harness` | [`b10x-harness`](https://github.com/beyond10x/harness) 0.22.0, an LLM endpoint, and `scan = "none"`, as for `codex` |
| a story | [`aep`](https://github.com/beyond10x/aep) and a planning store; a gates document for the gate ([Run a story](./run-a-story.md)) |
| `--forge local` | nothing beyond `git`: a bare repository on this machine as the tree's `origin`, and the programs its `local_checks` name |
| `--forge gitlab` | [`glab`](https://gitlab.com/gitlab-org/cli), with `glab auth status --hostname <host>` exiting 0, and `scan = "none"` in the run file |
| reading a case | [`jq`](https://jqlang.github.io/jq/), as [Read a case](./read-a-case.md) uses it |
| `takt promote` | [`bwrap`](https://github.com/containers/bubblewrap) on Linux, `/usr/bin/sandbox-exec` on macOS; and rustup's default toolchain |

A gate's commands are the exception: they find a program by name on `/usr/local/bin:/usr/bin:/bin`,
or on the `PATH` their `env` sets, as [Run a story](./run-a-story.md) says.

metaharness, the layer every harness runs through, is a library inside takt, not a program to
install.

## Claude Code's version

Before the first Claude Code session, takt asks metaharness whether the `claude` it would start
qualifies. takt 0.21.0 builds on metaharness 0.23.0, which qualifies Claude Code **2.1.304,
2.1.305 and 2.1.309**. Any other version stops the run before any model call, exit 1. With
Claude Code 2.1.312 on the `PATH` (the first three lines of output left out, the path written as
`~/takt-demo`):

```console
$ takt run --repo . --forge none --hours 1 --intent "add a function sub(left, right) that subtracts right from left, with a test"
  harness claude-code 2.1.312 · confinement none
stopped: HarnessUnqualified (claude 2.1.312: H11 unk: the launch imposes it over a scratch working directory; but there is no memory canary recorded for 2.1.312, so nobody measured whether this binary honours the control; qualified versions: 2.1.304, 2.1.305, 2.1.309; select another binary with --harness-program); steps 0, tokens 0, wall 4.0s, commits 0, case ~/takt-demo/.takt/cases/takt-20261029T110600Z
```

The case stays open; the next run resumes it. Name a qualified binary by its absolute path with
`--harness-program`, on every run (it is a flag only, with no key in the run file). The `opened`
event records its path, its sha256 and its `--version` line.

Claude Code's updater keeps its versions under `~/.local/share/claude/versions/` and removes old
ones. Copy a qualified version to a path of your own once:

```sh
mkdir -p ~/.local/opt/claude-2.1.309
install -m 755 ~/.local/share/claude/versions/2.1.309 ~/.local/opt/claude-2.1.309/claude
~/.local/opt/claude-2.1.309/claude --version
takt run --repo . --forge none --harness-program ~/.local/opt/claude-2.1.309/claude \
  --intent "add a function sub"
```

## Confinement

takt has no sandbox of its own. It starts the test command through
[Substrate](../concepts/glossary.md#substrate)'s confinement contract, on Linux and macOS
alike, and `--confinement` picks how:

| `--confinement` | the test command runs |
|---|---|
| `none`, the default | through Substrate's `local` adapter, declared insecure: on the host as you, with an environment cleared of logins, a scratch home and `TMPDIR`, a timeout and bounded output. It can read whatever you can. The run's first lines say so. |
| `substrate` | through the first adapter of the host that serves it: `systemd` around `linux`, then `linux` (bubblewrap), then `macos` (Seatbelt). The tree is the workspace, its `target/` written, the toolchain and cargo caches read-only, no network. What no adapter serves is refused by name, never run unconfined. |

The run's `case` line and its `opened` event name the adapter and the facts it proved. Under
`none`, a run's `case` line and the line after it read (the path shortened):

```console
case takt-20261029T110550Z on software.change/1 in ~/takt-demo; inputs risk=standard(default) change_kind=feature(default) affects_security_boundary=false(default); confinement none: Substrate local, declared insecure: the run was started with the default confinement none: its test command runs on the host as this user, unconfined; proved: declared_insecure, paths_are_host_paths, local_permitted, host_network
  harness none · confinement none
```

**The harness session is not confined by the operating system**, under either setting. Its
limits are metaharness's: it works in a scratch copy of the tree, with an environment built from
an allowlist, and Claude Code is offered only the file tools `Read,Edit,Write,Glob,Grep`
([A judgement step](../concepts/one-step-at-a-time.md#a-judgement-step)).

Under `substrate` on Linux, `target/` is the only path of the tree the test command writes; `.git`
and the rest are read-only, and a host whose probe did not prove `exec.workspace-scoped-write` is
refused by that name. A test run that would write `Cargo.lock` then fails, so commit it. On
`macos` the tree stays writable and its `.git` is sealed. `--confinement-cgroup-root <dir>` bounds
the `linux` adapter in a cgroup delegated to you, without `systemd`.

`takt promote` builds a candidate through the same selection. On macOS its build installs into
the workspace and takt copies the binary out. A `.cargo/config.toml` in a directory above the
build directory, `~/.cargo/config.toml` included, fails that build: the adapter lets cargo see the
file but not read it.
