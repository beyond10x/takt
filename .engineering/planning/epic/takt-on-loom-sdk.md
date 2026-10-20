---
format: aep.planning-md/3
id: epic:takt-on-loom-sdk
kind: epic
status: draft
title: 'takt on loom-sdk: Commission''s runtime drives the case, the governor decides, the harness proposes'
summary: loom-sdk's Commission runtime, governor and executor contract replace takt's case, decision, protocol and loop code; takt keeps validators, evidence, forge, scenarios, scorecard, promotion, trace
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# Epic: takt-on-loom-sdk

## Outcome

takt becomes an application on `b10x-loom-sdk` (the owner's decision of 2026-10-20). Commission's
runtime drives every case until it is blocked, and Loom's governor decides it with Canon. Claude
Code and, later, Codex are `AgentExecutor`s that propose; takt's effects perform. takt keeps what is
its own: validators and evidence, git and forge (commit, and merge under a grant), the planning
store seam (`crates/takt/src/frontier.rs`), scenarios, scorecard, promotion and improvement, trace,
report and view.

## Context

- Today the loop is takt's own (`crates/takt/src/run.rs`), and `docs/design/takt.md` rules out a
  runtime of its own besides it. This epic replaces takt's case, decision and protocol evaluation
  and the case loop with loom-sdk's Commission runtime and governor. Architecture decision: ADR
  0090 (Loom), and the owner's decision of 2026-10-20 that Loom reaches agent harnesses only
  through metaharness.
- An `AgentExecutor` proposes and never performs: Claude Code works in a scratch copy and proposes
  its change; takt's edit effect applies it under the step guard.
- The agent session executor (Claude and Codex over the metaharness library) is built here first
  as a standalone crate with no takt-only dependency, so it can move into Loom unchanged; Loom
  depends on metaharness, never the reverse.
- Loom's governor supplies no authority: takt's grants move to an `AuthorityProvider` over
  `authority.yaml`. That changes who decides a grant and needs the owner's grant when it lands.
- Gaps in loom-sdk that block a story are Loom's stories, not takt workarounds; a story they block
  stays blocked. The gap list is held outside this repository.
- `story:second-harness-adapter` and `story:confinement-seam` are rescoped into this epic.
- Open `rsi` stories are scheduled before this epic's (AGENTS.md § Rules).

## Scope

- `crates/takt/**`, `crates/takt-cli/**` (the loop, case, decision, protocol and harness code), a
  new session-executor crate, `Cargo.toml`, `Cargo.lock`.
- `ess/domains/takt.yaml` and its citations; `AGENTS.md` (Layout, Rules).
- `docs/design/takt.md`, `README.md`, `CHANGELOG.md`.

## Out of scope

- Loom's own model executor as takt's judgement actor; Loom's CLI, intake router and connectors.
- `workflow/1`, the case tree of `design:continuous-orchestration`.
- Any takt-side substitute for a loom-sdk gap.

## Acceptance

1. `cargo tree -p b10x-takt` shows `b10x-loom-sdk` at a Loom tag and one copy of each Canon and
   engineering-protocols crate.
2. `crates/takt/src` has no `b10x_canon::eval` call and no `decision.rs`; `run.rs` calls
   `run_until_blocked`; a test greps for both.
3. The Claude and Codex session executors implement `AgentExecutor` through the metaharness
   library; neither session's working directory is the case tree.
4. On the scratch crate with one failing test, `takt run --iterations 3` under each harness ends
   with `tests.pass` true and `repository.merge` waiting for its grant, with equal claim,
   obligation, action and outcome statuses.
5. Every evidence record of every fixture run has a validator as producer (or `takt.choice` from
   an effect); a test enforces it.
6. `merge_under_authority`, `judge_guard`, `judge_grant_bot`, `step_cannot_forge_its_validators`,
   `ignored_directories_are_guarded*`, `stop_kinds` and `run_record*` pass, every changed case
   named in its story.
7. `takt run --help` prints the same exit-code table, and each code is reached on its fixture.
8. `takt scorecard --against` the last promoted pre-epic baseline shows no regression over the
   full corpus under the promotion rule.
9. A case left at its merge grant by the last pre-epic release resumes after `takt approve` and
   merges.
