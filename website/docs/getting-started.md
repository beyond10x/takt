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
| any run | `git`; on Linux, unprivileged user namespaces (the test command runs in a user and mount namespace of its own); on macOS, `/usr/bin/sandbox-exec` (the test command runs under a Seatbelt profile) |
| the tests | the test command's toolchain; the default is `cargo test --quiet` |
| no model | `--harness none`: only the validators run |
| Claude Code | `claude` on the `PATH`, logged in (the default harness) |
| Codex or b10x-harness | `codex` or `b10x-harness` on the `PATH`, and [b10x-gates](https://github.com/beyond10x/gates) with a policy file in `B10X_GATES_POLICY`: required, not optional (below) |
| a story | `aep`, a planning store with a ready story, and a gates document ([Run a story](./guides/run-a-story.md)) |
| a pull request on github.com | `--forge bot`: the b10x-gates bot App, with `b10x-gates` configured for it, and `gh` to read the pull request |
| a merge request on a GitLab host | `--forge gitlab`: `glab` logged in to that host |
| no pull request | `--forge none` |
| `takt promote` | on Linux bubblewrap (`bwrap`), on macOS `/usr/bin/sandbox-exec`; and rustup's default toolchain |

### The gates scan is required for every harness but Claude Code

Codex and b10x-harness sessions may read outside the copy they work in, so takt accepts what they
propose only after `b10x-gates` has scanned it under the policy file `B10X_GATES_POLICY` names.
This is a hard requirement, not a check that is skipped when absent: with no policy the proposal
is refused, `not scanned, which is not passed`, and a scan that cannot run, `b10x-gates` missing
among them, refuses it as well. Claude Code's file tools
stay inside the copy, so a Claude Code session needs no policy. Every proposal, from any harness,
is also checked for the shape of a credential.

### The bot forge is github.com through the b10x-gates App

`--forge bot`, the default for a repository whose `origin` is on github.com, pushes the case's
branch, opens its pull request and merges it through `b10x-gates bot`, which acts as the b10x-gates
GitHub App, and reads the pull request's state and checks with `gh`. It serves github.com only and
is not a general forge adapter. A GitLab host is `--forge gitlab`; anything else runs with
`--forge none` ([Configuration](./reference/configuration.md#the-forge)).

:::note[macOS]

Since 0.10.0, takt confines on macOS with Seatbelt, `/usr/bin/sandbox-exec`, where Linux uses
namespaces and bubblewrap:

- The test command runs under a profile that writes only to the tree, its git directories (not
  their hooks or configuration), a fresh home that holds its `TMPDIR`, and the cargo caches. It
  reads no registry login, keychain, runtime directory or bus socket, and connects to no bus or
  launchd socket.
- A promotion's build, and its binary's `--version`, run under a profile with no network that
  writes only to the directories the build owns. `takt promote` runs on macOS as on Linux.
- With no `sandbox-exec`, both are refused by name and never run unconfined.

What differs from Linux:

- A promotion's build sees the clone, its target and the fresh root at their host paths, and may
  read every file outside the user's home.
- The test command's denials are by path: a hard link to a denied file elsewhere, and the keychain
  daemons reached over Mach, are not denied.
- A process that clears `TAKT_CHILD` from its environment and leaves takt's process group outlives
  the run; on Linux takt adopts and kills it. A harness session's program runs in takt's process
  group, not a session of its own.
- `--confinement substrate` runs on Linux only: loom-sdk's `SubstrateRunner` needs a delegated
  cgroup v2 scope, which takt gets from `systemd-run --user --scope -p Delegate=yes`.

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
