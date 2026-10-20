---
format: aep.planning-md/3
id: story:the-loop-is-run-until-blocked
kind: story
status: draft
title: takt run drives the case with run_until_blocked, and every stop keeps its exit code
summary: Composite executor (due validator first, else harness); stop and exit code derived from LoopEnd; no corpus regression
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:validators-perform-as-effects
revision: 1
---
# Story: the-loop-is-run-until-blocked

## Outcome

`takt run` drives its case with `run_until_blocked` of loom-sdk and a composite executor — the due validator first, else the harness — and derives its stop and exit code from the run's end. Every stop kind keeps its exit code and is reached on its fixture, and the scorecard over the full corpus does not regress.

## Context

- Today the loop is takt's own (`crates/takt/src/run.rs`); its stops are `STOP_KINDS` (`run.rs:120-131`) and `Completed`, mapped to exit codes 0-8 (`crates/takt-cli/src/main.rs:492-529`).
- `run_until_blocked` is at Loom 0.26.0 `crates/loom-commission/src/runtime.rs:578`, exported by loom-sdk (`crates/loom-sdk/src/lib.rs:70`). Without a step budget it ends after two idle iterations in a row and at an unbudgeted step limit (`runtime.rs:128-143`); it charges effects only (`runtime.rs:148-155`). Determinism: the governor completes a case only on exactly one legitimate outcome (`crates/loom-governor/src/lib.rs:74-76`).
- The harness is offered only the actions the frontier admits; the run record and the events stay comparable for promotion.
- A story blocked by a gap of loom-sdk stays blocked; takt builds no substitute (`epic:takt-on-loom-sdk` § Out of scope).

## Domain relations

- Run → Case: one — inferable (`ess/domains/takt.yaml`, `takt.work.Run` relation `case`).
- Run → Step: one-to-many, the run owns its steps — inferable (`ess/domains/takt.yaml`, `takt.work.Run` relation `steps`).

## Scope

- `crates/takt/src/run.rs` (the loop replaced; the composite executor; stop derived from the run's end)
- `crates/takt/src/events.rs` (events of the new loop)
- `crates/takt-cli/src/main.rs` (exit codes from the derived stop)
- `crates/takt/tests/stop_kinds.rs`, `crates/takt/tests/run_record*.rs`, `crates/takt/tests/hours_bounds_*.rs`
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv`

## Acceptance

1. `crates/takt/src/run.rs` calls `run_until_blocked`; a test greps for the call.
2. Each entry of `STOP_KINDS`, and `Completed`, is reached on a fixture through the new loop and exits with the code it has today; `stop_kinds` passes.
3. `takt run --help` prints the exit-code table it prints before this story, byte for byte.
4. `run_record*`, `hours_bounds_*` and `step_call_usage` pass; a harness step records its tokens and `--tokens` and `--hours` bound harness steps.
5. `takt scorecard --against` the last promoted baseline before the epic shows no regression over the full corpus under the promotion rule.

## Blocked by

- A host can end a run on an effect's refusal, with its reason; today a refused proposal counts as idle and the executor runs again (Loom 0.26.0 `crates/loom-commission/src/outcome.rs:66-70`, `runtime.rs:133-140`). Blocks `StepRefused` and exit 3.
- The run's outcome tells the idle bound apart from no admissible action; both end as `NoAdmissibleAction` today (`runtime.rs:132-134`; `generated/rust/commission/src/responsibility.rs:561-562`). Blocks telling `NoProgress` from `NothingAdmissible`.
- An effect can answer pending, observe again, and be invoked again within the budget without the iteration counting as idle (`runtime.rs:134-137`). Blocks `Waiting` and exit 6.
- `NoPerformableAction` names the actions and the open claim no performer serves; it carries no data today (`responsibility.rs:563-564`). Blocks `Unperformable` with its claim and performer, exit 7, and the candidate hand-over.
- An `AgentExecutor` that is not a model-port loop reports its usage — tokens, cost, wall time — into the run's charges, budget and meter; today only effects are charged (`runtime.rs:148-155`) and only model-port turns are metered (`crates/loom-executor/src/metering.rs:1-4`). Blocks `--tokens` and `--hours` over harness steps and per-step tokens.

## Out of Scope

Removing the old evaluation code (`story:takt-evaluation-code-retired`); `workflow/1` with an external harness.
