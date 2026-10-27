---
title: Getting started
sidebar_position: 2
description: Install takt 0.14.0, run it once with no model and no credentials, then once with a model, and pick the guide for what comes next.
lede: One run needs nothing but git and cargo; the second adds a model, and both leave a case directory you can read.
source: takt run --help and takt approve --help at 0.14.0; runs of takt 0.14.0 on fresh cargo crates, 2026-10-26; the install commands run against the 0.14.0 release assets
---

# Getting started

This page takes you from a download to two finished runs on a scratch crate. The first run needs
no model and no credentials. The second lets a model make one change.

## Install a release

Each version tag publishes `takt-<version>-<target>.tar.gz` for `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`, with a `SHA256SUMS` file, at
[github.com/beyond10x/takt/releases](https://github.com/beyond10x/takt/releases). The binary
needs no Rust toolchain to start, though a repository whose tests are cargo's needs one to test.

```sh
version=0.14.0 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
"takt-$version-$target/takt" --version
```

Put `takt` on your `PATH`. `takt --version` prints the version and the commit it was built from:

```console
$ takt --version
takt 0.14.0 (da165bb1383b30de345842c6e4c508e99bf38c4d)
```

macOS, `cargo install` from the tag, and the container image are in [Install](./guides/install.md).

## What it needs

| for | you need |
|---|---|
| any run | `git`; on Linux, unprivileged user namespaces (the test command runs in a user and mount namespace of its own) |
| the tests | the test command's toolchain; the default is `cargo test --quiet` |
| no model | `--harness none`: only the validators run |
| Claude Code | `claude` on the `PATH`, logged in (the default harness) |
| Codex or b10x-harness | `codex` or `b10x-harness` on the `PATH`, and `b10x-gates` with a gates policy in `B10X_GATES_POLICY` before an edit is accepted |
| a story | `aep`, a planning store with a ready story, and a gates document ([Run a story](./guides/run-a-story.md)) |
| a pull request | a forge: `b10x-gates` and `gh` for github.com, or `glab` logged in for a GitLab host; `--forge none` runs without one |
| `takt promote` | bubblewrap (`bwrap`) and rustup's default toolchain |

Codex and b10x-harness sessions may read outside the copy they work in, so takt accepts what they
propose only after `b10x-gates` has scanned it under a policy. Without `B10X_GATES_POLICY` the edit
is refused: `not scanned, which is not passed`. Claude Code's file tools stay inside the copy, so
it needs no policy.

:::warning[macOS]

bubblewrap is Linux only, so on macOS `takt promote` refuses with exit 1 before it judges;
`--dry-run` and `--baseline-only` still run. The test command's user and mount namespace is
Linux only too, and on macOS takt kills only what a child left in its own process group and
session.

:::

## A run with no model

Make a crate with one passing test and commit it, `Cargo.lock` included. A test run that writes a
lock file the commit lacks is refused as a change the validator may not make.

```sh
cargo new --lib takt-demo
cd takt-demo
cargo generate-lockfile
git add -A
git commit -m "start"
takt run --repo . --intent "keep the tests green" --harness none --forge none
```

Output, run in `~/takt-demo` (only that path was shortened):

```console
case takt-20261026T234353Z on software.change/1 in ~/takt-demo; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
step 2: repository.merge (none)
  said: no harness: nothing but the approval is left; requesting it
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 2, tokens 0, case ~/takt-demo/.engineering/cases/takt-20261026T234353Z
```

What happened:

1. takt opened a case on the protocol `software.change/1` and put the tree on the branch
   `takt/takt-20261026T234353Z`.
2. Each `frontier:` line is the governor's verdict on every action before a step. Only an
   admissible action may be taken.
3. Step 1 ran the test command as a validator and recorded its result as evidence.
4. With the tests green, the merge became `approval-required`. No harness was there to do more,
   so the run stopped for a grant, exit 4.

The case is a directory in the repository, `.engineering/cases/<case>/`, untracked so far.
[Read a case](./guides/read-a-case.md) walks through it. `takt approve` grants the merge, as
[Approve a capability](./guides/approve-a-capability.md) shows. Merging is a judgement step, so
it needs a harness.

## A run with a model

The same crate, a fresh copy, and a real change. This run used Codex:

```sh
B10X_GATES_POLICY=<policy file> takt run --repo . --harness codex --forge none \
  --intent "add a function sub(left, right) that subtracts right from left, with a test"
```

```console
case takt-20261026T233005Z on software.change/1 in ~/takt-demo2; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
step 2: implementation.edit (codex)
  said: Added sub(left, right) for u64 subtraction and a unit test; tests left for takt to run. [metaharness hermetic on: H1a unk, H1b ok, H2 ok, H3 ok, H4 unk, H5 unk, H6 ok, H7 ok, H8 ok, H9 ok, H10 ok, H11 ok]
  effect: committed 1 path(s); implementation is now aa38e5c5
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 3: tests.run {}
  effect: the test command exited 0: pass; 2 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
step 4: repository.merge (codex)
  said: Reviewed the diff: sub(left, right) subtracts right from left, with a test confirming sub(5, 2) equals 3. No further edits needed; requesting merge approval. [metaharness hermetic on: H1a unk, H1b ok, H2 ok, H3 ok, H4 unk, H5 unk, H6 ok, H7 ok, H8 ok, H9 ok, H10 ok, H11 ok]
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 4, tokens 75709, case ~/takt-demo2/.engineering/cases/takt-20261026T233005Z
```

The model took two steps: the edit and the request for the merge. takt committed the edit, then
its own validator ran the tests again on the new revision. The model never ran the tests itself,
and its word is not evidence. Without `--harness codex`, the same command runs Claude Code and
needs no `B10X_GATES_POLICY`. To use an LLM endpoint of your own through b10x-harness, see
[Configure a run](./guides/configure-a-run.md).

## Next

| to | read |
|---|---|
| set the harness, endpoint, forge and budgets in one file | [Configure a run](./guides/configure-a-run.md) |
| deliver a story from the planning store | [Run a story](./guides/run-a-story.md) |
| grant the merge and resume | [Approve a capability](./guides/approve-a-capability.md) |
| see why a run stopped | [How a run ends](./concepts/how-a-run-ends.md) |
| run it in a container or on Kubernetes | [Run in a container](./guides/run-in-a-container.md), [Deploy on Kubernetes](./guides/deploy-on-kubernetes.md) |
| know what does not work yet | [Known limitations](./known-limitations.md) |
