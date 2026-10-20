---
format: aep.planning-md/3
id: story:second-harness-adapter
kind: story
status: draft
title: A second harness adapter behind the Harness trait yields the same decisions
summary: The brief and report contract shared; a second program drives the judgement step
tags:
- loom
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-on-loom-sdk
- depends_on: story:takt-evaluation-code-retired
- depends_on: story:confinement-seam
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/tests/harness_seam.rs
revision: 8
---
# Story: second-harness-adapter

## Outcome

An operator switches the judgement actor between Claude Code and Codex with `--harness <name>` and gets the same decisions, because both are session executors in one crate that propose through metaharness and neither performs; what a case decides depends on evidence, never on which model took the step.

## Context

- Rescoped into `epic:takt-on-loom-sdk` (2026-10-20). `docs/design/takt.md` § 4 M4: a second harness adapter proves the seam. After `story:takt-evaluation-code-retired` the seam is the `AgentExecutor` of loom-sdk, not the `Harness` trait.
- `story:harness-proposes-takt-performs` built the session-executor crate for Claude: metaharness with `Kind::Claude`, `HermeticMode::Strict`, native tools, the scratch copy as working directory, no dependency on a takt-only crate so the crate can move into Loom unchanged (the owner's decision of 2026-10-20: Loom reaches agent harnesses only through metaharness, and Loom depends on metaharness, never the reverse). This story extends the same crate with Codex, `Kind::Codex` (metaharness 0.9.3 `crates/metaharness-protocol/src/spec.rs:19-23`); the brief and the proposal contract are shared, so both executors are given the same brief and held to the same proposal.
- "The same decisions" means the same claim values, obligation statuses, action statuses and outcome statuses per evaluation, not the same instants or commit ids.
- Tokens are charged for a Codex step only when Codex reports them; a step with no report charges none and the run record says so.
- `story:confinement-seam` comes first, so the Codex session starts through the same seam as the Claude session.

## Domain relations

- Executor call → proposal: one call per frontier yields exactly one outcome, which proposes and completes nothing — inferable (inferred from Loom 0.26.0 `crates/loom-commission/src/ports/executor.rs:1-5`; the proposal is declared in `ess/domains/takt.yaml` by `story:harness-proposes-takt-performs`).

## Scope

- `crates/<session-executor>/` (the Codex executor beside the Claude one; the shared brief and proposal)
- `crates/takt-cli/src/main.rs` (`--harness codex`)
- `Cargo.lock`
- new: `crates/takt/tests/harness_parity.rs` (a scripted metaharness session per kind proves the contract without a model)

## Acceptance

1. On the scratch crate with one failing test, `takt run --iterations 3` under `--harness claude` and under `--harness codex` both end with `tests.pass` true and `repository.merge` waiting for its grant, with equal claim, obligation, action and outcome statuses in every decision.
2. A scripted Codex session that proposes an action it was not offered is refused exactly as a Claude session is: `StepRefused`, exit 3, the case tree unchanged.
3. The environment and prompt the Codex executor hands metaharness carry none of the variables the run withholds from a harness; a test reads them.
4. A Codex step whose session reports no tokens charges none, and the run record shows its tokens as not reported.
5. `cargo tree -p <session-executor>` still lists no crate of this repository.

## Out of Scope

A hosted-model executor or a runtime of takt's own; confining the session (`story:harness-child-confined`); moving the crate into Loom; prompt tuning; the parity proof on a planning-store story, which left with `story:story-done-end-to-end` when this story was rescoped.
