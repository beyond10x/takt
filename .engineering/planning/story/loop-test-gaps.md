---
format: aep.planning-md/3
id: story:loop-test-gaps
kind: story
status: active
title: The loop's refusals and budgets are held by tests
summary: Kill the four surviving loop mutants; frontier tests through open_next
tags:
- wave-06
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/tests/frontier.rs
- confidence: inferred
  path: crates/takt/tests/loop_gaps.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 6}
---
# Story: loop-test-gaps

## Outcome

The loop's refusals and budgets are held by tests that fail when the behaviour goes, and the frontier's tests exercise the path production uses.

## Context

Findings of the 2026-10-18 code review, confirmed by an independent re-read of the mutation log:

| # | gap | where |
|---|---|---|
| 17 A | a `repository.inspect` step that changes a file is refused: mutant survives (the only inspect tests write into the cases directory, refused earlier) | `crates/takt/src/run.rs:631` |
| 17 B | a gate request with uncommitted changes is refused: mutant survives | `run.rs:612-625` |
| 17 C | `--tokens` budget: no test sets `RunOptions.tokens` | `run.rs:403` |
| 17 D | `--hours` budget: no test sets `RunOptions.hours` | `run.rs:398` |
| 18 | `Frontier::next` is unused in production; the "nothing ready on a second ask" test goes through `next`, not `open_next` | `crates/takt/src/frontier.rs:91-106`, `frontier.rs:172-178` |

17 E (`CiRead`) belongs to story:validator-confinement.

## Scope

- new: `crates/takt/tests/loop_gaps.rs` (inferred)
- `crates/takt/tests/frontier.rs` (cited)
- `crates/takt/src/frontier.rs`: remove `Frontier::next` if nothing in production needs it (inferred)

## Acceptance

Each of mutants A–D, applied by hand to the integration branch, turns at least one test red; the "nothing ready on a second ask" behaviour is asserted through `Run::open_next`; `Frontier::next` is either used by production code or gone.
