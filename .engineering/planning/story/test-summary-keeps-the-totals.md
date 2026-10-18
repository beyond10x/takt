---
format: aep.planning-md/3
id: story:test-summary-keeps-the-totals
kind: story
status: draft
title: The tests.run summary keeps the test command's totals
tags:
- finding
refs:
- provider: finding
  reference: test-summary-keeps-the-totals
relations:
- informed_by: review-result:variety-runs-a0b8576
revision: 1
---
## Outcome

The `tests.run` effect line and record keep the test command's own summary, not only its last five lines, so an operator and the model can tell how many tests ran.

## Context

From the planner's variety runs of 2026-10-18: the `tests.run` summary keeps the last five lines of stdout plus stderr (`crates/takt/src/validator.rs:281-305` on main, `:148` at a0b8576). In a Cargo workspace those are the doc-tests of the last crate, so every passing run read `running 0 tests … 0 passed` while 26 suites had passed (runs on worktree and metaharness). Reproduced on scenario 1 at 9922f12 with a test script whose first line is its summary and whose last six lines are filler: the effect line holds the filler and drops the summary.

```expected_before
scenario: 1
command: printf "#!/bin/sh\necho SUITES-26-PASSED\nfor i in 1 2 3 4 5 6; do echo tail-\$i; done\n" > "$HOME/t.sh" && chmod +x "$HOME/t.sh" && "$TAKT" run --repo "$SCENARIO" --intent probe --forge none --harness none --iterations 1 --test-cmd "$HOME/t.sh" 2>&1 | grep "effect: the test command" | grep -v SUITES-26-PASSED | grep -q tail-6 && echo summary-lost
output: summary-lost
source: .engineering/planning/review-result/variety-runs-a0b8576.md
```

## Scope

- `crates/takt/src/validator.rs`: the `tests.run` summary (cited)

## Acceptance

On the predicate's run the effect line holds `SUITES-26-PASSED`; for `cargo test` the summary keeps each `test result:` line or their totals.
