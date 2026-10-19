---
format: aep.planning-md/3
id: story:hours-bounds-a-harness-step
kind: story
status: implemented
title: --hours bounds a harness step, not only the gaps between steps
tags:
- finding
refs:
- provider: finding
  reference: hours-bounds-a-harness-step
relations:
- informed_by: story:cli-small-fixes
- supersedes: story:cli-small-fixes
- serves: vision:safe-agentic-coding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T20:13:54Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T20:13:54Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T04:04:07Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
## Outcome

`takt run --hours <h>` ends a harness step when the budget runs out: the harness child is given the remaining budget as its timeout, so no step runs past the run's end.

## Context

Split out of story:cli-small-fixes, item 1, found by an independent code review of 2026-10-18: `--hours` is checked between steps only (`crates/takt/src/run.rs:621-625`), and a harness step keeps its own 30-minute timeout (`crates/takt/src/harness.rs:73`). Reproduced by the planner on takt at main 9922f12 on scenario 1's seed, with a `claude` on `PATH` that sleeps 20 s: under `--hours 0.001` (3.6 s) the run took 21 s, stopping on step 2 with "the harness failed". It is the finding of the third recorded improvement turn: checkable offline by `scripts/check-finding`.

```expected_before
scenario: 1
command: mkdir -p "$HOME/b" && printf '#!/bin/sh\nsleep 20\n' > "$HOME/b/claude" && chmod +x "$HOME/b/claude" && s=$(date +%s); PATH="$HOME/b:$PATH" "$TAKT" run --repo "$SCENARIO" --intent probe --harness claude --forge none --iterations 3 --hours 0.001 --test-cmd ./check.sh; e=$(date +%s); [ $((e-s)) -ge 20 ] && echo overran
output: overran
source: .engineering/planning/story/cli-small-fixes.md
```

## Scope

- `crates/takt/src/run.rs`: the remaining budget passed to the harness step (cited)
- `crates/takt/src/harness.rs`: the step timeout (cited)
- `crates/takt/tests/`: a test with a harness that outlives the budget (inferred)

## Acceptance

With a harness that sleeps 20 s and `--hours 0.001`, `takt run` stops within the budget plus a fixed grace of at most 10 s, kills the harness child and discards its step's changes; `scripts/check-finding` on this finding prints `reproduced` at 9922f12 and `not_reproduced` at the fix.

## Landed (wave 12)

Read from `git diff --name-only 3b0b748^1 3b0b748` (the unit's merge into the wave branch) at the close of wave 12 (wave branch head 8d7c194). The hour budget bounds a harness step, the test command and the gate's commands; a step or observation returning at or after the run's end is discarded and the run stops on hours. Improvement turn 3. Test premises got room under load at 5e6f115, 9dea7ba and a931f5d.

- `crates/takt-cli/tests/hours_bounds_a_harness_step_cli.rs`
- `crates/takt/src/harness.rs`
- `crates/takt/src/run.rs`
- `crates/takt/src/validator.rs`
- `crates/takt/tests/ci_pending_waits.rs`
- `crates/takt/tests/hours_bounds_a_harness_step.rs`
- `crates/takt/tests/hours_bounds_a_harness_step_adv1.rs`
- `crates/takt/tests/hours_bounds_a_harness_step_adv2.rs`
- `crates/takt/tests/hours_bounds_the_run_end.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
