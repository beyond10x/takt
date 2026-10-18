---
format: aep.planning-md/3
id: story:validator-confinement
kind: story
status: implemented
title: Nothing a model writes becomes evidence, a grant or a gate definition
summary: Validators guarded, gates read from base, minimal child env, timeouts
tags:
- wave-06
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/validator_confinement.rs
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 9}
- {from: "active", to: "implemented", at: "2026-10-18T12:01:23Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
# Story: validator-confinement

## Outcome

Nothing a model writes can become evidence, a grant or a gate definition: validators run under the same case-directory guard as judgement steps, gate and project definitions are read from the base revision, and child processes get a minimal environment, a timeout and bounded output.

## Context

Findings of the 2026-10-18 code review, confirmed by independent probes:

| # | defect | where |
|---|---|---|
| 1 | validators run model-written code (the test command) with no case-directory guard and no producer check; a probe forged `authority.yaml` and ci/gate/merge records and merged with CI failing | `crates/takt/src/run.rs:485-491`, `crates/takt/src/validator.rs:93-98`, `crates/takt/src/case.rs:796-822` |
| 2 | the gates document, the project file and the planning store are read from the tree the model edits; the project file is looked up again at every observation | `validator.rs:193-216`, `validator.rs:195-199`, `crates/takt-cli/src/main.rs:257-263` |
| — | `gate.run` runs gate commands from that model-writable project file outside any guard | `run.rs:572-578` |
| 14 | `tests.run` passes the full environment (API keys, `GH_TOKEN`) to model code; the harness strips only `ANTHROPIC_API_KEY`, `CLAUDECODE`, `CLAUDE_PID`, `CLAUDE_CODE_*`, not `GH_TOKEN` | `validator.rs:94-98`, `crates/takt/src/harness.rs:280-287` |
| 15 | no child timeout, unbounded stdout buffering; `--hours` checked only between steps | `harness.rs:288-290`, `validator.rs:94-98`, `crates/takt/src/forge.rs:171-174`, `crates/takt/src/frontier.rs:229-233` |
| 24 | `claude -p` loads the repository's `.claude/settings.json` hooks (probe: a hook received the environment incl. `GH_TOKEN`) and the user's own settings and plugins | `harness.rs:258-273` |
| 25 | `gh pr view`/`gh pr checks` without `--repo` | `forge.rs:339-361` |
| 17 E | `CiRead`'s pushed-revision check has no test (surviving mutant) | `validator.rs:593` |

## Scope

- `crates/takt/src/validator.rs` (cited)
- `crates/takt/src/harness.rs`: `ClaudeCode` spawn and environment (around 150-300), not `is_test_path` (cited)
- `crates/takt/src/forge.rs` (cited)
- `crates/takt/src/run.rs`: the validator section of `Run::run` (around 476-540) only (cited)
- `crates/takt/src/frontier.rs`: the `aep` spawn (around 225-235) only (cited)
- new: `crates/takt/tests/validator_confinement.rs` (inferred)

## Acceptance

With a test command that writes a grant, an evidence record and a changed gates file into the tree: the run refuses the observation, removes what the command wrote, writes no record and merges nothing; a gate run reads `.engineering/gates.yaml` and the project file from the case's base revision, so a gate rewritten on the case branch does not change the verdict; the test command, the harness child, the forge and the frontier receive an environment without `GH_TOKEN`, `GITHUB_TOKEN`, `ANTHROPIC_API_KEY` and `GITLAB_*` tokens unless the adapter needs it (the forge keeps the bot route's), are killed after a timeout with the run stopping and naming it, and keep at most a bounded tail of their output; the harness child does not load project or user hooks; `gh pr view`/`checks` name `--repo`; a test kills the `CiRead` pushed-revision mutant.

## Landed (wave 06)

Read from `git diff --stat 9618c80 c0e8f4e` at the close of wave 06 (fix branch head 95995b4). The widened assignment of correction 1 added `git.rs` (`git::run` hardening), `case.rs` (`Branch.base_commit`) and `Cargo.toml`/`Cargo.lock` (`libc`).

- `Cargo.lock`
- `crates/takt/Cargo.toml`
- `crates/takt/src/case.rs`
- `crates/takt/src/forge.rs`
- `crates/takt/src/frontier.rs`
- `crates/takt/src/git.rs`
- `crates/takt/src/harness.rs`
- `crates/takt/src/run.rs`
- `crates/takt/src/validator.rs`
- `crates/takt/tests/forge.rs`
- `crates/takt/tests/frontier.rs`
- `crates/takt/tests/gate_validator.rs`
- `crates/takt/tests/tree_hygiene.rs`
- `crates/takt/tests/validator_confinement.rs`
- `crates/takt/tests/validator_confinement_adv1.rs`
- `crates/takt/tests/validator_confinement_adv2.rs`
