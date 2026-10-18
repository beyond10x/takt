---
format: aep.planning-md/3
id: story:trial-fetch-needs-no-user-ssh
kind: story
status: active
title: takt scenario run fetches trial branches without the user's SSH credential
summary: the trial-branch fetch hung 300 s twice because takt strips SSH_AUTH_SOCK and the bot fetch uses origin's SSH URL
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 5}
---
# Story: trial-fetch-needs-no-user-ssh

## Outcome

`takt scenario run` fetches the remote's trial branches without the user's SSH credential, and a fetch that cannot authenticate fails at once with the reason instead of waiting for a prompt until its timeout.

## Context

Observed by the operator on 2026-10-18, takt at 0a776d2, in the timed takt arm of the baseline comparison (fresh clones of the scratch repository, `origin` an `ssh://git@github.com/…` URL, a global `url.ssh://git@github.com/.insteadOf https://github.com/`):

- Two of two runs stopped before anything was cut: `scenario 1: fetch the remote's trial branches; nothing was changed: run b10x-gates bot -- fetch origin +refs/heads/trial/*:refs/remotes/origin/trial/* in <clone>: b10x-gates timed out after 300.0 s and was killed`, exit 1.
- The same line run by hand in the same clone, with the user's `SSH_AUTH_SOCK`, exits 0 in 2.1 s; with `SSH_AUTH_SOCK` removed and no terminal (`setsid`, standard input `/dev/null`) it exits 1 in 6.4 s with git's "Please make sure you have the correct access rights".
- takt removes `SSH_AUTH_SOCK` from every child (`crates/takt/src/forge.rs:384`, since cf3680d), on the rule that a push goes under the bot's credential (`forge.rs:388-389`). The bot route pushes over HTTPS under the App's credential, but `b10x-gates bot -- fetch` goes to `origin` as configured, so the fetch has no credential. That the child reaches a terminal and waits on an SSH prompt is inferred from the difference between 300 s and 6.4 s, not traced.
- Earlier scenario runs on the same machine fetched without error; why is not known (inferred: a reused SSH connection).

## Expected before

On takt at 0a776d2, holds; after the fix, fails:

- In a fresh clone of the scratch repository over SSH, with the user's SSH agent loaded, `takt scenario run --repo <clone> 1 --test-cmd ./check.sh --hours 1` exits 1 and its last line contains `fetch the remote's trial branches` and `timed out`.

## Scope

- `crates/takt-cli/src/main.rs`: `TrialRoute::fetch_argv`, `TrialRoute::fetch` (inferred)
- `crates/takt/src/forge.rs`: how a bot-route child is started (no terminal, `GIT_TERMINAL_PROMPT=0`, SSH batch mode) (inferred)

## Acceptance

In a fresh SSH clone with the user's agent loaded, `takt scenario run` fetches the trial branches through a route that needs no user credential (the bot route over HTTPS, or an anonymous fetch of a public repository) and proceeds to cut `trial/<n>`; a fetch that cannot authenticate exits within 30 s naming the remote and the reason, with nothing changed.
