---
format: aep.planning-md/3
id: story:loom-sdk-pinned-with-evaluation-parity
kind: story
status: implemented
title: takt depends on loom-sdk at a tag, and the Loom governor decides fixture cases as takt does
summary: b10x-loom-sdk pinned at a Loom tag; governor::evaluate renders the same decision bytes on fixtures that read no grant, and the granted difference is asserted
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: .engineering/cases
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/takt/Cargo.toml
- confidence: inferred
  path: crates/takt/tests/fixtures/loom-parity
- confidence: cited
  path: crates/takt/tests/loom_parity.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T05:46:17Z", actor: "human:timo", revision: 9}
- {from: "proposed", to: "active", at: "2026-10-20T05:46:17Z", actor: "human:timo", revision: 10}
- {from: "active", to: "implemented", at: "2026-10-20T06:22:55Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1}}}
---
# Story: loom-sdk-pinned-with-evaluation-parity

## Outcome

`b10x-loom-sdk` is a dependency of `b10x-takt` at a Loom tag, and a test proves that the governor of Loom, evaluating a takt case, decides it as takt decides it today. Every later story of `epic:takt-on-loom-sdk` can then replace takt's evaluation without changing what a case decides. Nothing in a run changes yet.

## Context

- First story of the epic, in strangler order; outside its test nothing calls loom-sdk yet.
- Today takt evaluates a case itself: `CaseDir::evaluate` (`crates/takt/src/case.rs:2323-2355`) hands Canon the protocol IR, `case.yaml`, the evidence records and the authority read from `authority.yaml` (`case.rs:2339-2347`), and writes `decisions/NNNN.json` (`case.rs:2352`).
- The governor of Loom evaluates with no authority supplied: Loom 0.26.0, `crates/loom-governor/src/lib.rs:54-56` (it never invents authority) and `:1742-1744`; `evaluate` is at `:1763` and is reachable as `loom_sdk::governor::evaluate` (`crates/loom-sdk/src/lib.rs:54`). Parity therefore holds on decisions that read no grant. On a granted fixture the two differ: an action takt shows granted is `approval-required` in the governor's decision. This story asserts that difference rather than hiding it; `story:governor-holds-takts-cases` decides where grants go.
- Loom 0.27.0 moves its governor to engineering-protocols 0.9.0, the version takt pins; Canon and
  engineering-protocols are the same commits in both lockfiles. `evaluate` is at
  `crates/loom-governor/src/lib.rs:1879` there. Its decision's `canon` is a parsed value with no
  serializer, so parity is compared on parsed values, not bytes (amended by the dispatcher,
  2026-10-20).
- The fixtures are the 30 decisions recorded under `.engineering/cases/turn-*/decisions/` (17 read
  no grant, 13 do), each under its case's own `protocol.yaml`, registered with
  `ProtocolCatalog::add_yaml`; each request is rebuilt from the decision's
  `explanation.computed_from` at its recorded instant.
- Open `rsi` stories are scheduled before this one (`AGENTS.md` § Rules).

## Domain relations

- Case → decision: one-to-many, the case owns its decisions, one per evaluation, numbered in order — inferable (inferred from `crates/takt/src/case.rs:2349-2353`, the write of `decisions/<seq>.json`; `ess/domains/takt.yaml` `takt.work.Case` names the decisions in its summary and declares no relation for them).

## Scope

- `Cargo.toml`, `Cargo.lock` (the workspace dependency at a Loom tag)
- `crates/takt/Cargo.toml`
- new: `crates/takt/tests/loom_parity.rs` (the parity test over existing fixtures)

## Acceptance

1. `cargo tree -p b10x-takt` lists `b10x-loom-sdk` at a Loom tag and exactly one copy each of `b10x-canon` and `b10x-canon-engineering`.
2. `cargo test -p b10x-takt --test loom_parity` passes: for every fixture decision that reads no grant, the decision `loom_sdk::governor::evaluate` returns equals, as a parsed value, the one `CaseDir::evaluate` wrote for the same protocol, case and evidence, at the recorded instant.
3. The same test holds one granted fixture: the decision of takt shows the action granted, the decision of the governor shows it `approval-required`, and the test asserts exactly the differences that follow from the missing grant (the action's status and reasons, and the decision's `explanation.computed_from.authority`) and no other.
4. `task check` passes, and the diff changes no file under `crates/takt/src` or `crates/takt-cli/src`.

## Blocked by

- Nothing once Loom 0.27.0 is tagged; until then, the tag.

## Out of Scope

Any call to loom-sdk from `crates/takt/src`; where grants go (`story:governor-holds-takts-cases`).
