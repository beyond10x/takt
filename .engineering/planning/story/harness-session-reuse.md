---
format: aep.planning-md/3
id: story:harness-session-reuse
kind: story
status: draft
title: Judgement steps of one case reuse one harness session
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:no-inspect-for-stateless-harness
revision: 1
---
## Outcome

The judgement steps of one case run in one harness session (or a resumed one), so the model does not re-read the repository at each step and each step does not pay a cold process start.

## Context

docs/runs/2026-10-18-efficiency-analysis.md: the Claude adapter runs each step as a new process under `--no-session-persistence` (`crates/takt/src/harness.rs` at a0b8576 :189): 3 processes per run cost 4.8 s of start-up, and the second process spends 47k tokens to add two tests, re-reading what the first had read. story:no-inspect-for-stateless-harness covers the extra inspect step; the cost of the cold sessions has no story. The step guard and the per-step checkpoint must hold across a reused session.

## Scope

- `crates/takt/src/harness.rs`: session flags and the step call (cited)
- `crates/takt/src/run.rs`: the harness step (inferred)

## Acceptance

On scenario 1, n=5 runs at the candidate against the promoted baseline: median tokens lower than the baseline's minimum, no outcome worse, and a step that writes outside its action is still refused.
