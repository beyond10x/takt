---
format: aep.planning-md/3
id: story:takt-uses-loom-session-executor
kind: story
status: draft
title: takt drives harness sessions through Loom's session executor and carries no copy of its own
summary: crates/session-executor removed; b10x-loom-session-executor from Loom's tag (ADR 0098 § 3)
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:takt-evaluation-code-retired
revision: 1
---
# Story: takt-uses-loom-session-executor

## Outcome

takt drives each harness session through Loom's `b10x-loom-session-executor` crate and carries no copy of its own: `crates/session-executor` is gone, and `crates/takt` depends on the crate Loom releases.

## Context

- ADR 0098 § 3 (`architecture/adr/0098-loom-calls-llm-for-decisions-and-metaharness-for-agent-work.md:57-59`): the session executor lives in Loom and moves there unchanged (ADR 0099).
- Two copies exist today. takt's is `crates/session-executor` (package `b10x-session-executor`, `crates/session-executor/Cargo.toml:2`), a path dependency of `crates/takt` (`crates/takt/Cargo.toml:19`). Loom 0.31.0's is `crates/loom-session-executor` (package `b10x-loom-session-executor`), which pins metaharness 0.10.1 as takt does (`Cargo.toml:18-19`).
- The two have diverged: `diff` of the two `src/lib.rs` files prints 1006 changed lines. Loom's records each session as an `AgentSession` of its `loom.session` domain and charges the session's usage to the run; takt's does neither.
- takt uses it in `crates/takt/src/harness.rs` (`session_executor::SessionExecutor::new` at `harness.rs:427`, and `Endpoint`, `Proposal`, `ProposedFile`, `Kind`, `CredentialSource`, `ProcessRunner`) and in `crates/takt/tests/harness_proposes.rs` and `harness_parity.rs`.
- `ess/domains/takt.yaml` cites `crates/session-executor/src/lib.rs` (`takt.yaml:2508-2509`, `5237`, `5242`), and `AGENTS.md:10` names `b10x-session-executor`.

```expected_before
scenario: 1
command: grep -c '^b10x-session-executor = { path = "../session-executor" }' crates/takt/Cargo.toml
exit: 0
output: 1
source: crates/takt/Cargo.toml:19
```

```expected_before
scenario: 2
command: test -d crates/session-executor
exit: 0
output: ""
source: crates/session-executor/Cargo.toml:2
```

## Scope

- `crates/session-executor/` (removed)
- `Cargo.toml`, `Cargo.lock`, `crates/takt/Cargo.toml`
- `crates/takt/src/harness.rs`, `crates/takt/src/run.rs`
- `crates/takt/tests/harness_proposes.rs`, `crates/takt/tests/harness_parity.rs`
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv` (citations re-pointed to the Loom crate at its pinned tag)
- `AGENTS.md` (§ Serves O3, § Layout)

## Acceptance

1. `test -d crates/session-executor` exits 1, and `cargo tree -p b10x-takt -i b10x-loom-session-executor` names the crate from Loom's tag that `b10x-loom-sdk` is pinned to.
2. `cargo test -p b10x-takt --test harness_proposes --test harness_parity` passes.
3. A harness step's session is recorded by Loom's executor: the case's `observations.json` after one `--harness claude` step with a stub runner holds the executor's `AgentSession` record.
4. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass, with no citation into `crates/session-executor`.
5. `task check` passes.

## Out of Scope

- Changes to Loom's crate. A gap found there goes to Loom's store through its delivery session.
- The metaharness pin: it stays the one Loom's crate pins.
