---
format: aep.planning-md/3
id: story:pr-scan-policy
kind: story
status: draft
title: pr.open scans the pull-request body under the forge's policy
summary: scan-text before a pull request gets the forge's --policy; a refusal after the push says the branch left
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/tests/forge_scan_policy.rs
revision: 4
---
# Story: pr-scan-policy

## Outcome

`pr.open` through the bot forge scans the pull-request body under the same policy file the API write uses, so a story run on a machine with no `~/.config/b10x/gates/policy.json` opens its pull request, and an error after the branch push no longer says that nothing left the machine.

## Context

Found by the operator's live trial on the scratch repository (scenario 1, `story:remove-a-todo`), takt at 92b8cf3, 2026-10-18. The run took the story under `story.delivery/1`, passed tests and the gate twice, pushed the case branch `takt/story-remove-a-todo-20261018T120340Z` to `origin`, then stopped with exit 5 at its 8th step:

`stopped: error: pr.open (takt.pr-open): open the pull request for takt/story-remove-a-todo-20261018T120340Z through the forge bot: the pull request did not pass scan-text; nothing left the machine: b10x-gates scan-text <case>/artifacts/pull-request.request.json exited 1: b10x-gates: protected file <home>/.config/b10x/gates/policy.json unavailable: not found`

Two defects:

1. `BotForge::scan_argv` (`crates/takt/src/forge.rs:683-689`) builds `b10x-gates scan-text <path>` with no `--policy`, while `api_argv` (`forge.rs:708-711`) appends the forge's `--policy` when one is set. `b10x-gates scan-text` takes `--policy` (env `B10X_GATES_POLICY`) and falls back to the default path, which does not exist on a machine whose policy lives elsewhere.
2. The context `the pull request did not pass scan-text; nothing left the machine` (`forge.rs:851`) is false at that point: the validator pushes the case branch before it calls `open_pull_request` (`crates/takt/src/validator.rs:723-725`). Only the body stayed local.

## Expected before

On takt at 92b8cf3, each holds; after the fix, each fails:

- `cargo test -p b10x-takt --test <the story's test> scan_argv_carries_the_forge_policy` fails, because `scan_argv` on a `BotForge` with `policy = Some(p)` contains no `--policy p`.
- Scenario 1 run with no `B10X_GATES_POLICY` in the environment and no default policy file (`takt run --repo <scratch> --test-cmd ./check.sh --forge bot --hours 1`) exits 5 and its last line contains `did not pass scan-text; nothing left the machine` and `protected file`.

## Scope

- `crates/takt/src/forge.rs`: `BotForge::scan_argv`, `BotForge::open_pull_request` (cited)
- `crates/takt/tests/`: a forge argv test (inferred)

## Acceptance

`scan_argv` carries the forge's `--policy` and `--repository` whenever `api_argv` does, and scenario 1 run with the policy given only through the forge reaches `pr.open` and opens the pull request; a scan refusal after the push names the pushed branch instead of saying nothing left the machine.
