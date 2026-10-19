---
title: Getting started
description: Install takt 0.1.0, check what it needs, and run one validated step.
---

# Getting started

## Install a release

Each version tag publishes `takt-<version>-<target>.tar.gz` for `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`, with a `SHA256SUMS` file, at
[github.com/beyond10x/takt/releases](https://github.com/beyond10x/takt/releases). No Rust
toolchain is needed.

```sh
version=0.1.0 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
"takt-$version-$target/takt" --version
```

On macOS, where `sha256sum` may be missing:

```sh
grep " takt-$version-$target.tar.gz\$" SHA256SUMS | shasum -a 256 --check
```

`takt --version` prints the version and the commit the binary was built from.

From source:

```sh
cargo install --locked --path crates/takt-cli
```

## What it needs

- A harness: Claude Code (`claude` on the `PATH`), or `--harness none` to run validators only.
- For story runs: `aep` on the `PATH` and its planning store in the repository
  (`.engineering/project.yaml`) with ready stories.
- For pull requests: an `origin` on github.com, and `b10x-gates` and `gh` on the `PATH`. Another
  host gets no forge, and `--forge none` runs without one. The forge's gates policy comes from
  `B10X_GATES_POLICY`.
- For `takt promote`: bubblewrap (`bwrap`) on the `PATH` and rustup's default toolchain. The
  candidate is built and run inside bubblewrap.

:::warning[macOS]

bubblewrap is Linux only, so on macOS a promotion refuses with exit 1 before it judges;
`--dry-run` and `--baseline-only`, which build nothing, still run. A run on macOS also kills only
what a child process left in its own process group; on Linux takt adopts and kills what the child
orphaned outside it as well. No command sandboxes the harness or the test command on either
system yet.

:::

## One step

On a repository with one failing test:

```sh
takt run --repo . --intent "make the failing test pass" --iterations 8 --input risk=trivial
```

The case opens on `software.change/1`. The first time this ran, it stopped after 4 steps on
`ApprovalRequired (repository.merge)`: the tests validator recorded the failure, the harness
edited the code, the validator recorded the pass, and the merge waited for a grant. The run
record is
[`docs/runs/2026-10-18-m1-scratch.md`](https://github.com/beyond10x/takt/blob/main/docs/runs/2026-10-18-m1-scratch.md).

Next: [run a story](./guides/run-a-story.md) from the planning store, then
[approve and resume](./guides/run-a-story.md#approve-and-resume) it.
