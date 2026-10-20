---
format: aep.planning-md/3
id: story:a-run-refuses-a-scanned-identity-at-open
kind: story
status: draft
title: A run refuses a commit identity the forge's scan rejects when the run opens, not at pr.open
summary: a trial set spent 368,818 tokens before the identity scan refused every run at pr.open
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-run-refuses-a-scanned-identity-at-open

## Outcome

A run whose commits leave the machine checks its commit identity with the scan the forge applies
before anything is pushed, when the run opens: an identity the scan refuses ends the run before
step 1, naming the scan's refusal and spending no tokens, instead of at `pr.open` after every
step has run.

## Context

The trial cycle on release 0.4.0 passed `--commit-identity` with a person's name and a noreply
address; the bot route's scan refuses that text, so every run of its first set took about 70,000
tokens of steps and was then refused at `pr.open` (five runs, 368,818 tokens, discarded). takt's
default identity, `takt <takt@localhost>`, passes the scan. The defect is when takt refuses, not
what. Comes from self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt-cli/src/main.rs` (the open-time identity refusal of story:takt-commits-with-the-repositorys-identity)
- `crates/takt/src/forge.rs` (the scan the bot route already runs at `pr.open`)
- `crates/takt-cli/tests/commit_identity_cli.rs`

## Acceptance

1. `takt run` and `takt scenario run` through the bot route with `--commit-identity X`, where the
   forge's scan refuses X, exit 1 before step 1, naming the scan's refusal; no step runs, no
   harness is called, nothing is pushed: a test with a scan double that refuses X.
2. The same for an identity read from `.engineering/commit-identity.conf`.
3. An identity the scan passes opens the run as before; under `--forge none` no scan is run.
4. No scan found (no policy) is reported as "not scanned", never as passed, and the run opens as
   it does today at `pr.open`.
