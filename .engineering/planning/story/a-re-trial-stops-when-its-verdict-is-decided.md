---
format: aep.planning-md/3
id: story:a-re-trial-stops-when-its-verdict-is-decided
kind: story
status: implemented
title: A re-trial runs on after its verdict is decided, and no run's active time is capped
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-re-trial-stops-when-its-verdict-is-decided
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:01:42Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-20T04:01:42Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-20T05:16:23Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# A re-trial runs on after its verdict is decided, and no run's active time is capped

## Outcome

A re-trial stops as soon as no result of its remaining runs can change its verdict, a run that
passes a token cap stops there and counts as at least the cap, and a run's active time (its wall
time less the time it waits on the forge) has a budget of its own, so a candidate clearly over its
baseline fails fast and a long forge queue does not stop a run that is otherwise on course.

## Context

From the trial of takt 0.3.0 (bee77822) and its first candidate, scenario 1, rules the owner set on
2026-10-20:

- Re-trial 1 of the candidate was decided after its fourth run: 3 of 4 runs were above the
  promoted baseline's median of 66,421 tokens, so the median of 5 could not fall below it, and none
  was above the saved maximum of 84,088. Its fifth run cost 88,252 tokens and changed nothing.
- The candidate's first run waited 1,140 s on a queued CI job (50 steps, 1,322 s wall); its
  `ci.read` steps cost 0 tokens and the run ended `ApprovalRequired` with the held-out check
  passing. A cap on wall time (`--hours`) would have cut a run that was on course and cost a rerun
  of about 67,000 tokens.
- `takt run` and `takt scenario run` have budgets of steps (`--iterations`), wall hours (`--hours`)
  and tokens (`--tokens`), and none for active time.

Rules:

1. **Stop rule.** After each run, the re-trial stops when no result of the remaining runs can
   change its verdict (better, not better, worse). At n = 5: three runs at or over the target
   median decide "not better"; two runs with a worse outcome decide "worse" (the rule's margin is
   one case).
2. **Token cap.** Each run is capped at the saved maximum of the scorecard it is compared with
   (`--tokens`); a capped run is not resumed and counts as at least the cap.
3. **Active time.** No cap on forge waits. A run's active time is capped at three times the
   baseline's longest active time; a run over it stops and counts as worse.
4. **Every set stops early.** Every trial set, first sets included, stops after two runs with
   the same worse outcome.
5. **Pre-flight.** Before run 1 of a set, a check that calls no model: free disk, usage headroom,
   `b10x-gates scan-text` over the configured commit identity and a sample commit message, a
   bot-route `push --dry-run`, and the revision of the binary under test. A failed check starts no
   run. Evidence: the trial cycle on release 0.4.0 discarded five runs (368,818 tokens), each
   refused at `pr.open` by the identity scan, which a pre-flight would have caught before run 1.

Rule 1 and the use of rules 2 and 3 are the trial agent's procedure (`.agents/trial-agent.md`
§ The cycle, step 8). Rule 3 needs a budget takt does not have.

```expected_before
scenario: 1
command: XDG_STATE_HOME="$HOME/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --harness none --forge none --test-cmd ./check.sh --iterations 1 --active-seconds 600
exit: 2
healthy_exit: 3
output: unexpected argument '--active-seconds'
source: .agents/trial-agent.md
```

At bee77822 `--active-seconds` does not parse, exit 2. Once the budget exists the same command
runs one validator step and stops `Iterations (1)`, exit 3.

## Scope

- `crates/takt-cli/src/main.rs`, `crates/takt/src/run.rs`: `--active-seconds <n>` on `takt run`
  and `takt scenario run`, counting a run's time less its waits on the forge; over it the run stops
  on a budget, exit 3, naming the budget, the active time and the wait time.
- `.agents/trial-agent.md` § The cycle, steps 2 and 8: rules 1 to 5 as the trial's procedure,
  the pre-flight before run 1 of every set.
- `crates/takt/tests/`: a run with a forge wait longer than its active budget is not stopped by
  the wait; a run whose steps pass the active budget stops on it.

## Acceptance

At the candidate the predicate gives `not_reproduced` (exit 3), and the trial agent's profile names
the five rules, the pre-flight's checks named one by one.
