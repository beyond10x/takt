---
title: Turns
description: The improvement turns so far, each with its finding, its checks, its scorecard and its deviations, and the first turns takt drove to accepted itself.
---

# Turns

There are two kinds of turn so far:

| turns | who recorded the evidence | where | outcome |
|---|---|---|---|
| 1 to 3, 2026-10-18 to 2026-10-19 | the agents, by hand (phase R0) | takt's `main`, the operator's installed takt | accepted, promoted |
| self-improvement trials 2 to 4, 2026-10-19 | takt's validators, every claim | scratch state, `--forge none` | accepted, promoted |

`docs/metrics/improvement-turns.jsonl` holds turns 1 to 3. The trials are in their run record,
[`docs/runs/2026-10-19-self-improvement-trials/`](https://github.com/beyond10x/takt/tree/main/docs/runs/2026-10-19-self-improvement-trials).

## Self-improvement trials 2 to 4, 2026-10-19: accepted on validators' records

takt drove each turn from open to `accepted` itself. In each of the three trials, all six claims
of `self.improvement/1` were recorded by a validator:

| claim | validator | its record |
|---|---|---|
| finding filed | `takt.finding-file` | `finding_record` filed |
| reproduced at the baseline | `takt.scenario-reproduce` | `scenario_result` reproduced |
| not reproduced at the candidate | `takt.scenario-verify` | `scenario_result` not_reproduced |
| merged into the mainline | `takt.candidate-merge-judge-surface` | `merge_observation` merged |
| no regression | `takt.scorecard-compare` | `scorecard_result` no_regression |
| promoted | `takt.binary-promote` | `promotion_record` promoted |

The finding was the same in all three: `takt scorecard --help` said a run's outcome comes from
`events.jsonl` only, but a scenario run's comes from the held-out check
(`story:scorecard-help-names-the-held-out-check`). In each trial, takt delivered the candidate
itself, on `story.delivery/1` with Claude Code as the harness.

| measure | trial 2 | trial 3 | trial 4 |
|---|---|---|---|
| takt (branch `impl/rsi-part-2`) | 71f061c, part 2 | 7a4d6b9, correction 1: the judge binary recorded at open | 30bd2f0, correction 2: the judge-surface rule, `--install-root` |
| candidate delivered by takt | 123 s, 268,051 tokens | 171 s, 491,390 tokens | 111 s, 285,325 tokens |
| scorecard, median tokens, baseline and candidate (n = 5 each) | 205,558 and 205,918 | 221,639 and 205,785 | 205,994 and 205,790 |
| turn, opened to accepted | 19 min 11 s | 4 min 17 s | 4 min 10 s |
| trial, first command to accepted | 28 min 12 s | 10 min 17 s | 9 min 46 s |
| tokens, every harness call of the trial | 2,750,162 | 2,628,726 | 2,450,830 |
| final resume run by | the trial's build | the kept judge binary | the kept judge binary |

Durations come from each trial's `timeline.tsv`, from the first row's start to the end of the
resume that accepted. Tokens are the sum of the harness transcripts' totals: one delivery and ten
scenario runs. The turn's own steps cost none, since it runs with `--harness none`. Trial 2 took
longer for two reasons. Its first baseline runs were aborted and run again, adding 239,745 tokens
to the figure above. And its candidate runs started only after the merge, so the resume waited
for their scorecard. Trial 1, at RSI part 1 (75e59c1), got as far as the verify step and stopped
there: that revision had no merge validator.

The probes in trials 3 and 4 ran on copies of a case and tried to break the turn. None did:

- a resume by the newly promoted binary: refused, exit 1, nothing run, because the turn is judged
  by the binary that opened it;
- a judge-surface grant `--by agent:developer` with no delegation at `main` (file absent, or
  `delegations: []`): no merge recorded, exit 3;
- `--install-root` on a resumed case: refused, exit 1, nothing resumed;
- the kept judge run with `CARGO_INSTALL_ROOT` pointing elsewhere: it still read the install root
  the case recorded, and accepted.

**What is not shown yet.** Every trial used a scratch promotion store, a scratch install root,
`--forge none`, and a fast-forward of `main` done by the operator. None of the candidates is on
takt's `main`. The operator's installed takt has not yet promoted a revision through a turn. The
trials' scenario runs ended on `NoProgress`, in an origin with no forge, so their tokens do not
compare with the scenario 1 figures on [Runs](./runs.mdx).

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
`docs/runs/2026-10-19-improvement-turns-2-and-3/README.md`,
`docs/runs/2026-10-19-self-improvement-trials/README.md`, `docs/metrics/README.md`.
