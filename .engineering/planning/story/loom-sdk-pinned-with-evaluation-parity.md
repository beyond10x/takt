---
format: aep.planning-md/3
id: story:loom-sdk-pinned-with-evaluation-parity
kind: story
status: draft
title: takt depends on loom-sdk at a tag, and the Loom governor decides fixture cases as takt does
summary: b10x-loom-sdk pinned at a Loom tag; governor::evaluate renders the same decision bytes on fixtures that read no grant, and the granted difference is asserted
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: loom-sdk-pinned-with-evaluation-parity

## Outcome

`b10x-loom-sdk` is a dependency of `b10x-takt` at a Loom tag, and a test proves that the governor of Loom, evaluating a takt case, decides it as takt decides it today. Every later story of `epic:takt-on-loom-sdk` can then replace takt's evaluation without changing what a case decides. Nothing in a run changes yet.

## Context

- First story of the epic, in strangler order; outside its test nothing calls loom-sdk yet.
- Today takt evaluates a case itself: `CaseDir::evaluate` (`crates/takt/src/case.rs:2323-2355`) hands Canon the protocol IR, `case.yaml`, the evidence records and the authority read from `authority.yaml` (`case.rs:2339-2347`), and writes `decisions/NNNN.json` (`case.rs:2352`).
- The governor of Loom evaluates with no authority supplied: Loom 0.26.0, `crates/loom-governor/src/lib.rs:54-56` (it never invents authority) and `:1742-1744`; `evaluate` is at `:1763` and is reachable as `loom_sdk::governor::evaluate` (`crates/loom-sdk/src/lib.rs:54`). Parity therefore holds on decisions that read no grant. On a granted fixture the two differ: an action takt shows granted is `approval-required` in the governor's decision. This story asserts that difference rather than hiding it; `story:governor-holds-takts-cases` decides where grants go.
- Loom 0.26.0 is the latest Loom tag on 2026-10-20. Its governor pins engineering-protocols 0.8.0 (`crates/loom-governor/Cargo.toml:19` there); takt pins 0.9.0 (`Cargo.toml:13-15`). Both pin Canon 0.6.0.
- Open `rsi` stories are scheduled before this one (`AGENTS.md` § Rules).

## Domain relations

- Case → decision: one-to-many, the case owns its decisions, one per evaluation, numbered in order — inferable (inferred from `crates/takt/src/case.rs:2349-2353`, the write of `decisions/<seq>.json`; `ess/domains/takt.yaml` `takt.work.Case` names the decisions in its summary and declares no relation for them).

## Scope

- `Cargo.toml`, `Cargo.lock` (the workspace dependency at a Loom tag)
- `crates/takt/Cargo.toml`
- new: `crates/takt/tests/loom_parity.rs` (the parity test over existing fixtures)

## Acceptance

1. `cargo tree -p b10x-takt` lists `b10x-loom-sdk` at a Loom tag and exactly one copy each of `b10x-canon` and `b10x-canon-engineering`.
2. `cargo test -p b10x-takt --test loom_parity` passes: for every fixture decision that reads no grant, the bytes `loom_sdk::governor::evaluate` renders equal the bytes `CaseDir::evaluate` writes for the same protocol, case and evidence.
3. The same test holds one granted fixture: the decision of takt shows the action granted, the decision of the governor shows it `approval-required`, and the test asserts exactly that difference and no other.
4. `task check` passes, and the diff changes no file under `crates/takt/src` or `crates/takt-cli/src`.

## Blocked by

- Acceptance 1 needs the governor of Loom on engineering-protocols 0.9.0, the version takt pins, so the tree holds one copy; Loom 0.26.0 is on 0.8.0. takt does not pin down to 0.8.0 as a workaround.

## Out of Scope

Any call to loom-sdk from `crates/takt/src`; where grants go (`story:governor-holds-takts-cases`).
