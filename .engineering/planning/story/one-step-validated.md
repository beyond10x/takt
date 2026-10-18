---
format: aep.planning-md/3
id: story:one-step-validated
kind: story
status: archived
title: 'M1: one step validated on software.change/1'
summary: takt run with an intent; a Claude Code step; tests.run as evidence; the decision printed
relations:
- decomposes: epic:composable-agentic-work
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T01:43:14Z", actor: "human:timo", revision: 3}
---
## What

`takt run --repo R --intent "<text>" --iterations N` opens a Canon case on `software.change/1`
(the built-in of `b10x-canon-engineering` 0.9.0), runs `tests.run` as a validator that writes a
`canon-evidence/1` `test_result` record bound to `HEAD`, lets Claude Code (`claude -p`, file tools
only) perform one admissible action per step in the worktree, commits the change as the effect,
moves the `implementation` artifact to the new `HEAD`, and prints the frontier, each step, each
evidence record and the stop reason.

## Acceptance

On a scratch crate with one failing test, `takt run --intent "make the failing test pass"` ends
with `tests.pass` TRUE, `repository.merge` approval-required and the stop line
`ApprovalRequired (repository.merge)`, and the case directory holds no evidence record whose
producer is the harness.

## Scope

- `crates/takt/src/{case,decision,evidence,git,harness,protocol,run,validator}.rs`
- `crates/takt-cli/src/main.rs`
