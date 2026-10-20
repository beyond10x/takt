---
format: aep.planning-md/3
id: story:a-step-replays-from-its-recorded-state
kind: story
status: implemented
title: One step of a recorded case cannot be replayed from its recorded state
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-step-replays-from-its-recorded-state
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:01:42Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T04:01:42Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T05:16:24Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# One step of a recorded case cannot be replayed from its recorded state

## Outcome

`takt replay` runs one judgement step of a recorded case again from the state that step started
from (the tree at its revision, the case's evidence, decisions and prompt inputs up to that step),
in a scratch copy, writing a record of its own and leaving the case untouched. A candidate that
targets one step is then measured on that step, several times, before any full scenario run.

## Context

From the trial of takt 0.3.0 (bee77822) and its first candidate, scenario 1, a rule the owner set
on 2026-10-20: before the five runs of a re-trial, a candidate that targets one step (there, the
`change.handover` step, step 9) has that step replayed alone from a recorded state, five times, and
the full scenario runs only if the step meets the candidate's own predicate (there: one model
request in at least 3 of 5).

- The candidate's first re-trial cost five full runs (58,010 to 88,252 tokens each) to learn that
  step 9 handed over in one request in 1 of 5 runs. Steps 3 and 6, which the candidate did not
  target, carried most of the spread (24,547 to 36,752 and 25,480 to 35,484 tokens).
- takt has no command or flag that replays a step: at bee77822 and at the mainline of 2026-10-20,
  `crates/takt-cli/src/main.rs` names no replay of a step, and `takt run --case` only resumes a
  case from its last state.

```expected_before
scenario: 1
command: "$TAKT" replay --help
exit: 2
healthy_exit: 0
output: unrecognized subcommand 'replay'
source: crates/takt-cli/src/main.rs
```

At bee77822 `replay` is no subcommand, exit 2. Once it exists its help prints, exit 0.

## Scope

- `crates/takt-cli/src/main.rs`: `takt replay --case <dir> --step <n> [--times <k>]`.
- `crates/takt/src/run.rs`, `crates/takt/src/case.rs`: rebuild a step's starting state from the
  case's record (the revision the step started at, the evidence and decisions before it), run the
  step's judgement in a scratch worktree, record it beside the case and never in it.
- `crates/takt/tests/`: a replayed step sees the prompt the recorded step saw, byte for byte, and
  leaves the case's files unchanged.

## Acceptance

At the candidate the predicate gives `not_reproduced` (exit 0), and a replay of scenario 1's step 9
from a recorded case gives the recorded step's prompt.
