---
format: aep.planning-md/3
id: story:cli-protocol-defaults
kind: story
status: active
title: A plain takt run on a store story delivers it through a pull request
summary: Store stories default to story.delivery/1, forge refusals stop, distinct exit codes
tags:
- wave-06
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/src/run.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 6}
---
# Story: cli-protocol-defaults

## Outcome

A plain `takt run` on a repository with a planning store delivers the story on the protocol that opens a pull request, a resumed case says which protocol it runs, a forge refusal ends the run with a `stopped:` line, and exit codes tell "merged" from "waiting for approval".

## Context

From two live trial runs on 2026-10-18 and the 2026-10-18 code review:

| source | defect | where |
|---|---|---|
| trial run 1 | a story taken from the store runs on the default `software.change/1`, which has no gate, pull-request or CI actions: the run stops at `ApprovalRequired (repository.merge)` with no push and no pull request | `crates/takt-cli/src/main.rs:66`, `main.rs:223-226` |
| trial run 1 | `Run::resume` ignores `--protocol` without saying so; a wrong-protocol case can only be abandoned by hand | `crates/takt/src/run.rs:290-330` |
| trial run 2 | a forge error (HTTP 422) ends the run with exit 1 and an error instead of a `stopped:` line | `crates/takt/src/run.rs` (validator observe error path), `main.rs` |
| review 23 | `--forge` help says the default is `bot` whenever there is an origin; the code requires github.com | `main.rs:103-108`, `main.rs:281-300` |
| review 30 | `Completed` and `ApprovalRequired` both exit 0 | `main.rs:147-153`, `main.rs:343-346` |

The story-delivery protocol ships in this repository (`protocols/story-delivery/1.yaml`); the binary embeds it so a target repository need not carry it.

## Scope

- `crates/takt-cli/src/main.rs` (cited)
- `crates/takt/src/run.rs`: `Run::resume` (around 290-335) and the top-level error-to-stop mapping only (inferred)
- `crates/takt/src/protocol.rs` (built-in lookup of the embedded story-delivery protocol) (inferred)
- `crates/takt-cli/tests/` (inferred)

## Acceptance

`takt run --repo R` with no `--protocol` on a repository whose planning store has a ready story opens the case on `story.delivery/1`; with `--intent` it stays `software.change/1`; `--protocol P` on a resumed case whose protocol differs exits non-zero naming both protocols and touching nothing; a forge that refuses `open_pull_request` ends the run with a `stopped:` line naming the refusal and a documented exit code; `ApprovalRequired` exits with a code different from `Completed`; `takt run --help` describes `--forge`'s default as the code chooses it.
