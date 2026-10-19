---
format: aep.planning-md/3
id: story:a-budget-admits-a-step
kind: story
status: draft
title: A budget that admits no step is refused before the story is taken
tags:
- finding
refs:
- provider: finding
  reference: a-budget-admits-a-step
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

`--iterations 0`, `--tokens 0` and an `--hours` too small to admit one step are refused before anything is written, as `--hours 0` already is: no story is taken, no commit is made.

## Context

From the planner's operator-path variety round on takt at 12cffde (`.engineering/planning/review-result/variety-ops-a29d8c7.md`). `--hours 0`, negative, NaN and infinity are refused ("a finite number above 0", `crates/takt-cli/src/main.rs:502`), but `--iterations 0`, `--tokens 0` and `--hours 1e-9` are accepted: each commits "takt: take story", leaves the story `active` and runs 0 steps (`crates/takt/src/run.rs:2866-2877`, `:2741`).

```expected_before
scenario: 1
command: "$TAKT" run --repo . --harness none --forge none --iterations 0 --test-cmd "sh check.sh"
exit: 3
output: stopped: Iterations (0); steps 0
source: .engineering/planning/review-result/variety-ops-a29d8c7.md
```

## Scope

- `crates/takt-cli/src/main.rs`: the budget flags (cited)
- `crates/takt/src/run.rs`: taking the story (cited)

## Acceptance

`--iterations 0` and `--tokens 0` exit 1 with the reason and leave HEAD and the story unchanged; `--hours` below a floor the help states exits 1 the same way.
