---
title: Turns
description: The improvement turns recorded so far, each with its finding, its checks, its scorecard and its deviations.
---

# Turns

`docs/metrics/improvement-turns.jsonl` holds one turn.

## Turn 1, 2026-10-18: accepted

On 2026-10-18 one improvement turn went through every claim of `self.improvement/1`, and Canon
returned `accepted`. The defect was small: `takt scenario run --help` listed an exit-code meaning
that only `takt run` has (`story:scenario-run-help-exit-codes`).

| claim | how it was earned |
|---|---|
| reproduced | `scripts/check-finding` printed `reproduced` at the baseline 0a776d2 |
| fixed | and `not_reproduced` at the candidate 447423c |
| no regression | 3 baseline runs (T-1..3) and 3 candidate runs (T-7..9) of scenario 1, all at the merge approval with the held-out check passing |
| merged | at 67ab6ce, into main at b670264, under a judge-surface grant |
| promoted | the binary built at a0b8576, tied to it by its sha256 |
| outcome | `accepted`, at the case's decision `0008.json` |

The candidate's median tokens were 151,626 against the baseline's 122,438, +24%, n = 3 each. Two
candidate runs (T-7, T-9) hit the known `repository.inspect` `NoProgress` stop. The candidate
changes only help text, so the difference is put down to that defect; that is an inference. The
design's first token default (median at most baseline + 10%) would have failed on these figures.
The rule on the [Scorecard](./scorecard.md) page was decided after this turn and replaced it.

The run record lists six deviations from the protocol as designed:

1. The records were written by hand, by a recorder program; no takt run drove the case (phase R0).
2. The scorecard was recorded before the merge could be.
3. The planner, not the reviewer, produced the checks.
4. Tokens came from a shim, not from the case's events.
5. Promotion was tied by sha256, since the binary embedded no revision then (it has since wave 11).
6. The recorder ran outside the repository, built against wave 10 code that was not in main then.

The run record and the case files:
[`docs/runs/2026-10-18-improvement-turn-1/`](https://github.com/beyond10x/takt/tree/main/docs/runs/2026-10-18-improvement-turn-1).

## Turn 2: candidate runs only

The finding is `story:case-id-is-a-valid-branch-name`. Its candidate ran 5 times at d74da0c and,
after review, 5 times at 98719c3, against 5 baseline runs at a0b8576 ([Runs](./runs.mdx)). The turn
itself is not recorded: `improvement-turns.jsonl` has no line for it.

Sources: `docs/metrics/improvement-turns.jsonl`, `docs/runs/2026-10-18-improvement-turn-1/README.md`,
`docs/metrics/README.md`.
