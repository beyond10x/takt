---
title: Turns
description: The improvement turns recorded so far, each with its finding, its checks, its scorecard and its deviations.
---

# Turns

`docs/metrics/improvement-turns.jsonl` holds three turns, all accepted.

## Turn 1, 2026-10-18: accepted

The turn went through every claim of `self.improvement/1`. The defect: `takt scenario run --help`
listed an exit-code meaning that only `takt run` has (`story:scenario-run-help-exit-codes`).

| claim | how it was earned |
|---|---|
| reproduced | `scripts/check-finding` printed `reproduced` at the baseline 0a776d2 |
| fixed | `not_reproduced` at the candidate 447423c |
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

## Turns 2 and 3, 2026-10-18 to 2026-10-19: accepted

Turn 2's defect: `takt run --case-id 'a~b'` passed takt's own id check and then failed inside
`git checkout -b` (`story:case-id-is-a-valid-branch-name`). Turn 3's: `--hours` was checked only
between steps, so a harness step could run past the budget (`story:hours-bounds-a-harness-step`).

| claim | turn 2 | turn 3 |
|---|---|---|
| reproduced | at the baseline a0b8576 | at the baseline a0b8576 |
| fixed | `not_reproduced` at the candidate 98719c3 | `not_reproduced` at the candidate 9fb14cc |
| no regression | T-24..28: median 122,465 tokens against the baseline's 122,361 (T-14..18), +0.1% | T-39..43: median 122,151 against the same baseline, -0.2% |
| merged | at 0cb62c0, into main at 06e3227 | at 76be9ed, into main at 06e3227 |
| promoted | 44f96a4, one promotion for both turns | the same |
| outcome | `accepted` | `accepted` |

Turn 2's first candidate, d74da0c (T-19..23), was superseded after review ([Runs](./runs.mdx)).
Both turns were recorded by hand in phase R0, as turn 1 was. The run record lists the other
deviations, among them that the `takt.promote` grant was recorded after the promotion and that
the candidate's own build saved the scorecard it was promoted on:
[`docs/runs/2026-10-19-improvement-turns-2-and-3/`](https://github.com/beyond10x/takt/tree/main/docs/runs/2026-10-19-improvement-turns-2-and-3).

Sources: `docs/metrics/improvement-turns.jsonl`, `docs/runs/2026-10-18-improvement-turn-1/README.md`,
`docs/runs/2026-10-19-improvement-turns-2-and-3/README.md`, `docs/metrics/README.md`.
