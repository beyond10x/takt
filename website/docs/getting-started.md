---
title: Getting started
sidebar_position: 2
description: Install takt 0.23.0, run it on a repository with no model, then with Claude Code, grant the merge and take the change.
lede: Two runs on your repository, the first with no model and no account, the second with Claude Code; then the grant and the change.
source: runs of a build of takt 0.21.0's source on a fresh cargo crate with Claude Code 2.1.312 and 2.1.309, takt approve and git merge --ff-only, 2026-10-29; takt run --help and takt approve --help of 0.21.0; the install commands as on the Install page
---

# Getting started

This page takes you from a download to a change made by Claude Code, checked by takt and taken
into your branch.

## What it needs

| for | you need |
|---|---|
| any run | Linux (x86_64 or aarch64) or macOS on Apple silicon; `git`; a repository with a test command, committed and clean |
| the tests | the test command's toolchain. The default command is `cargo test --quiet`; give another with `--test-cmd` |
| the model | [Claude Code](https://github.com/anthropics/claude-code), logged in, at 2.1.304, 2.1.305 or 2.1.309 ([why](./guides/install.md#claude-codes-version)) |

Codex, b10x-harness, a forge or a planning store need more: [the tools beside takt](./guides/install.md#the-tools-beside-takt).

## Install

```sh
version=0.23.0 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
mkdir -p ~/.local/bin
install -m 755 "takt-$version-$target/takt" ~/.local/bin/takt
takt --version
```

On Linux aarch64 the target is `aarch64-unknown-linux-gnu`; on macOS, `aarch64-apple-darwin`
and `shasum` instead of `sha256sum`. Both, and `cargo install`: [Install](./guides/install.md).

:::caution[What runs unconfined]

By default the test command runs on your host as you, unconfined, and takt says so on the run's
first lines. The Claude Code session is held by no operating-system sandbox: it works in a scratch
copy of your tree and is offered only file tools. `--confinement substrate` confines the test
command: [Confinement](./guides/install.md#confinement).

:::

## 1. A run with no model

Run it in your repository; the tree must be committed and clean. Here it is a fresh crate:

```sh
cargo new --lib takt-demo
cd takt-demo
cargo generate-lockfile
git add -A
git commit -m "start"
takt run --repo . --intent "keep the tests green" --harness none --forge none
```

`--harness none` takes no model step. `--forge none` opens no pull request; without it, takt
picks a forge from `origin`: `bot` for github.com, `gitlab` for a host `glab` is logged in to
([forge](./concepts/glossary.md#forge)). In a Rust repository, commit `Cargo.lock`: a test run
that writes a lock file the commit lacks is refused, and the refusal names the ways out.

Output (the path written as `~/takt-demo`):

```console
settings: protocol software.change/1 (default); risk=standard (default); change_kind=feature (default); affects_security_boundary=false (default)
takt 0.21.0 · keep the tests green
case takt-20261029T110550Z on software.change/1 in ~/takt-demo; inputs risk=standard(default) change_kind=feature(default) affects_security_boundary=false(default); confinement none: Substrate local, declared insecure: the run was started with the default confinement none: its test command runs on the host as this user, unconfined; proved: declared_insecure, paths_are_host_paths, local_permitted, host_network
  harness none · confinement none
  may: implementation.edit repository.inspect tests.run tests.write
step 1: tests.run {} · pass · 0.8s
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
step 2: none working (may: implementation.edit tests.write; may ask: repository.merge)
step 2: repository.merge (none)
  said: no harness: nothing but the approval is left; requesting it
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 2, tokens 0, wall 2.6s, commits 0, case ~/takt-demo/.takt/cases/takt-20261029T110550Z
```

What happened:

1. takt opened a [case](./concepts/glossary.md#case) on the
   [protocol](./concepts/glossary.md#protocol) `software.change/1` and switched the tree to the
   branch `takt/<case>`.
2. `may:` lists the actions the run may take now, the
   [frontier](./concepts/glossary.md#frontier) said briefly. `--verbose` prints the whole
   frontier at every step.
3. Step 1 ran your test command as a [validator](./concepts/glossary.md#validator) and recorded
   its result as [evidence](./concepts/glossary.md#evidence) about this commit.
4. With the tests green, the merge could be asked for. It needs a
   [grant](./concepts/glossary.md#grant), so the run stopped, exit 4.

The case is the directory `.takt/cases/<case>/`. takt wrote `.takt/.gitignore` holding `*`, so
`git status` shows none of it. [Read a case](./guides/read-a-case.md) walks through it.

## 2. A run with Claude Code

Start from a clean tree on your base branch again (`git switch main`, or a fresh copy). The
default [harness](./concepts/glossary.md#harness) is Claude Code. If the `claude` on your `PATH` is not a qualified version, name
one with `--harness-program` ([Install](./guides/install.md#claude-codes-version)):

```sh
takt run --repo . --forge none --hours 1 \
  --harness-program ~/.local/opt/claude-2.1.309/claude \
  --intent "add a function sub(left, right) that subtracts right from left, with a test"
```

`--hours 1` stops the run after an hour at most. Output, the first three lines left out and the
path written as `~/takt-demo`:

```console
  harness claude-code 2.1.309 · confinement none
  may: implementation.edit repository.inspect tests.run tests.write
step 1: tests.run {} · pass · 0.3s
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
step 2: claude-code working (may: implementation.edit tests.write; may ask: repository.merge)
step 2: implementation.edit (claude-code) · 22.0s · 37.9k tok (27.7k cached) · $0.11
  said: Added sub(left, right) to src/lib.rs returning the exact difference as i128, with a unit test covering positive, negative and extreme cases
  Glob **/*
  Read src/lib.rs
  Read Cargo.toml
  Read .takt/cases/takt-20261029T110612Z/decisions/0002.json
  Edit src/lib.rs
  Edit src/lib.rs
    diff --git a/src/lib.rs b/src/lib.rs
    index b93cf3f..a86171d 100644
    --- a/src/lib.rs
    +++ b/src/lib.rs
    @@ -2,6 +2,10 @@ pub fn add(left: u64, right: u64) -> u64 {
         left + right
     }
     
    +pub fn sub(left: u64, right: u64) -> i128 {
    +    left as i128 - right as i128
    +}
    +
     #[cfg(test)]
     mod tests {
         use super::*;
    @@ -11,4 +15,12 @@ mod tests {
             let result = add(2, 2);
             assert_eq!(result, 4);
         }
    +
    +    #[test]
    +    fn sub_works() {
    +        assert_eq!(sub(5, 3), 2);
    +        assert_eq!(sub(3, 5), -2);
    +        assert_eq!(sub(0, u64::MAX), -(u64::MAX as i128));
    +        assert_eq!(sub(u64::MAX, 0), u64::MAX as i128);
    +    }
     }
  effect: committed 1 path(s); implementation is now c3cda2f5: src/lib.rs
step 3: tests.run {} · pass · 0.6s
  effect: the test command exited 0: pass; 2 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
step 4: claude-code working (may: implementation.edit tests.write; may ask: repository.merge)
step 4: repository.merge (claude-code) · 11.7s · 14.5k tok (11.4k cached) · $0.04
  said: sub(left, right) returns left - right as i128, correct for all u64 inputs including negative results; tests cover both signs and the u64 extremes
  Glob *.md
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 4, tokens 52345, wall 41.7s, cost $0.15, commits 1 (1 path), case ~/takt-demo/.takt/cases/takt-20261029T110612Z
```

The model took two steps: the edit and the request for the merge. Under each `said:` line are
the tools it called, then the diff takt committed for it. takt then ran the tests again itself
on the new commit. The model never ran the tests, and its word is not evidence.
`--quiet-steps` leaves the tool calls and the diff out.

## 3. Grant the merge and take the change

Grant the merge for the case the run printed:

```console
$ takt approve --repo . --by human:me takt-20261029T110612Z repository.merge
approved: repository.merge for case takt-20261029T110612Z by human:me, written to takt-20261029T110612Z/authority.yaml
recorded: nothing in the planning store; the case delivers no story (it was opened on an intent, or is an improvement turn)
```

The merge is a validator step, not a model step: takt performs it through a forge, under the
grant. With `--forge none` there is no forge to merge into, so a resumed run merges nothing (it
stops `NoProgress`, exit 3). The change is the commit on the branch `takt/<case>`; take it with
git:

```sh
git switch main
git merge --ff-only takt/takt-20261029T110612Z
```

To have takt merge, run with a forge. `--forge local` needs no account: a bare repository on
this machine is the forge ([Run a story](./guides/run-a-story.md#on-this-machine-the-local-forge)).
To resume an intent's case, name it: `takt run --repo . --case <case>`.

## Other harnesses and settings

Codex and b10x-harness need a scan of what they propose, a gates policy or `scan = "none"` in a
run file: [Configure a run](./guides/configure-a-run.md).

## Next

| to | read |
|---|---|
| look up a word | [Glossary](./concepts/glossary.md) |
| set the harness, forge, test command and budgets in one file | [Configure a run](./guides/configure-a-run.md) |
| deliver a story from an AEP planning store | [Run a story](./guides/run-a-story.md) |
| grant a capability and resume | [Approve a capability](./guides/approve-a-capability.md) |
| see why a run stopped | [How a run ends](./concepts/how-a-run-ends.md) |
| run it in a container | [Run in a container](./guides/run-in-a-container.md) |
| know what does not work yet | [Known limitations](./known-limitations.md) |
