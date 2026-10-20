---
format: aep.planning-md/3
id: story:harness-proposes-takt-performs
kind: story
status: implemented
title: Claude Code proposes through a metaharness session executor, and the edit effect of takt applies it under the step guard
summary: A standalone session-executor crate implements AgentExecutor over metaharness (Kind::Claude, hermetic Strict, native tools, cwd a scratch copy)
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:loom-sdk-pinned-with-evaluation-parity
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: inferred
  path: crates/session-executor
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/Cargo.toml
- confidence: inferred
  path: crates/takt/src/edit.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/tests/harness_proposes.rs
- confidence: cited
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: inferred
  path: fuzz/Cargo.lock
revision: 19
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T14:29:02Z", actor: "human:timo", revision: 17}
- {from: "proposed", to: "active", at: "2026-10-20T14:29:02Z", actor: "human:timo", revision: 18}
- {from: "active", to: "implemented", at: "2026-10-20T14:29:02Z", actor: "human:timo", revision: 19, decided_on: {"recorded":{"test_result":1}}}
---
# Story: harness-proposes-takt-performs

## Outcome

The judgement step's model works in a scratch copy of the case tree, never in the tree itself, and returns its change as a proposal. takt's edit effect applies the proposal to the case tree under the step guard, so a step that does more than the frontier admits is refused before a byte of it reaches the tree.

## Context

- Today `ClaudeCode` (`crates/takt/src/harness.rs:110`) runs `claude -p` in the case tree; the step guard reverts afterwards what it should not have done, and the loop refuses an action it did not offer (`Stop::StepRefused`, `crates/takt/src/run.rs:69`).
- In loom-sdk an `AgentExecutor` proposes and never performs (Loom 0.26.0, `crates/loom-commission/src/ports/executor.rs:1-5`); only the runtime admits a request, and an `EffectPort` performs it (`crates/loom-commission/src/ports/effect.rs:1-8`). The Claude Code port of Loom is a model port with tools off (`crates/loom-executor/src/wire/claude_code.rs:3-19`), so it does not replace the harness of takt.
- The owner's decision of 2026-10-20: Loom reaches agent harnesses only through metaharness. The executor runs Claude through the metaharness library — `Kind::Claude`, `HermeticMode::Strict`, Claude's native tools, the scratch copy as working directory — not through a `claude -p` spawn of its own.
- The executor is a standalone crate with no dependency on `b10x-takt` or any other takt-only crate, so it can move into Loom unchanged; Loom depends on metaharness, never the reverse. `story:second-harness-adapter` extends the same crate with Codex.
- The stability page of loom-sdk does not yet list `AgentExecutor` among the ports a host implements (Loom `website/docs/reference/sdk-stability.md:57-79`); the crate builds on it as it stands at the pinned tag.
- Spec first (`AGENTS.md` § Rules): the proposal — the action it names and the files it changes, as paths relative to the tree and their contents — is declared in `ess/domains/takt.yaml` before code builds on it.
- The loop stays takt's own here (`crates/takt/src/run.rs`): the executor is called at the harness call site; `story:the-loop-is-run-until-blocked` moves the call into the Commission runtime.

## Domain relations

- Executor call → proposal: one call per frontier yields exactly one outcome, which proposes and completes nothing — inferable (inferred from Loom 0.26.0 `crates/loom-commission/src/ports/executor.rs:1-5`; no ess/1 document of takt declares it yet, and this story declares the proposal).
- Run → Step: one-to-many, the run owns its steps — inferable (`ess/domains/takt.yaml`, `takt.work.Run` relation `steps`).

## Scope

- new: `crates/<session-executor>/` (its `Cargo.toml` and sources; the implementer names it, with no `b10x-takt` dependency)
- `Cargo.toml`, `Cargo.lock`
- `crates/takt/src/harness.rs`, `crates/takt/src/run.rs` (the proposal applied by the edit effect under the step guard)
- `crates/takt-cli/src/main.rs` (`--harness claude` selects the executor)
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv` (the proposal)
- new: `crates/takt/tests/harness_proposes.rs`

## Acceptance

1. `cargo tree -p <session-executor>` lists `metaharness` and `b10x-loom-sdk` and no crate of this repository.
2. The executor implements the `AgentExecutor` of loom-sdk. A test with a scripted metaharness session shows the session's working directory is a scratch copy, not the case tree, and that the case tree is byte-identical after the session until the edit effect applies the proposal.
3. A scripted proposal that names an action the frontier did not offer, or changes a path the step guard refuses, ends the run with `StepRefused` and exit 3, and leaves the case tree unchanged.
4. The environment and the prompt the executor hands metaharness carry none of the variables the run withholds from the harness today (`crates/takt/src/harness.rs:660-664`); a test reads them.
5. On the scratch crate with one failing test, `takt run --harness claude --iterations 3` ends with `tests.pass` true and `repository.merge` waiting for its grant.
6. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass with the proposal declared.

## Blocked by

- metaharness 0.9.3 refuses `HermeticMode::Strict` with a working directory the embedder names
  (`crates/metaharness/src/builder.rs:200-205` there; the Claude launch rows H7 and H11 are
  unavailable), and this story runs the session under Strict in a scratch copy. metaharness must
  accept Strict with an embedder's scratch working directory first; running under
  `HermeticMode::On` instead would weaken isolation and is not taken.

## Out of Scope

Codex (`story:second-harness-adapter`); confining the session (`story:harness-child-confined`); the Commission loop (`story:the-loop-is-run-until-blocked`); moving the crate into Loom.
