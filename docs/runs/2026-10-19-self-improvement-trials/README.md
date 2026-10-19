# Run record: the first improvement turns takt drove to `accepted`, 2026-10-19

Four trials ran a `self.improvement/1` turn with takt itself as the driver, each in a scratch
state directory. In trials 2, 3 and 4 the turn reached `accepted` and its candidate was promoted,
and a validator recorded every claim: the finding filed, reproduced on the baseline, not
reproduced on the candidate, merged into the mainline, no regression on the scorecard, promoted.
Nobody wrote a record by hand, as was done for turns 1 to 3 in phase R0
(`docs/runs/2026-10-18-improvement-turn-1/`, `docs/runs/2026-10-19-improvement-turns-2-and-3/`).
Trial 1 got as far as the verify step and stopped there: that revision had no merge validator.

**What was scratch.** Each trial had its own promotion store, a state directory of its own
(`XDG_STATE_HOME`). It had its own install root too, seeded with the trial's own build of takt.
Every turn ran under `--forge none`. The operator fast-forwarded the scratch clone's `main` to
the candidate (`git fetch . <candidate branch>:main`), and the merge validator read that local
`main`. Each grant was given by a scratch owner id. The operator's own installed takt has not yet
promoted a revision through a turn, and none of the candidates is on takt's `main`: the finding,
`story:scorecard-help-names-the-held-out-check`, is still a draft there.

The trial records stay outside the repository because they hold local paths: per trial,
`logs/timeline.tsv` (one row per command: start time, name, exit code, seconds, command),
`logs/notes.tsv`, one log per command, the harness transcripts with their analyses
(`logs/transcripts-*-analysis.txt`), and the case directory. Every figure below is read from them.
The row or file is named beside it. Times are UTC.

## The trials

| | trial 1 | trial 2 | trial 3 | trial 4 |
|---|---|---|---|---|
| takt revision (`impl/rsi-part-2` unless stated) | `75e59c1` (RSI part 1, on `main`) | `71f061c`, part 2 | `7a4d6b9`, part 2 correction 1 | `30bd2f0`, part 2 after correction 2 and the merge of 0.2.0 |
| finding | `story:scorecard-help-names-the-held-out-check` | the same | the same | the same |
| candidate | `f0f7f50` | `c9e78ab` | `8cafc53` | `3a5d8f3` |
| outcome | error at `candidate.deliver`: "the case declares no artifact implementation" (`22-turn-with-intent.log`) | `accepted`, decision `0077.json` | `accepted`, decision `0015.json` | `accepted`, decision `0015.json` |
| turn, opened to accepted | | 12:49:45 to 13:08:56, 19 min 11 s | 13:37:43 to 13:42:00, 4 min 17 s | 14:36:35 to 14:40:45, 4 min 10 s |
| trial, first row of `timeline.tsv` to the end of the last resume | | 12:40:44 to 13:08:56, 28 min 12 s | 13:31:43 to 13:42:00, 10 min 17 s | 14:30:59 to 14:40:45, 9 min 46 s |

The turn's span runs from the start of the `turn-open` row to the final decision's `computed_from`
time. The trial's span runs from the first row's start to the end (start plus seconds) of the
`turn-resume` row that accepted. Trial 2's `worktree-finish` row ends at 13:09:27, which makes
28 min 43 s. Trial 2 took longer for two reasons. Its first baseline runs were aborted, because
they all shared one case id (`notes.tsv`, 12:46:30). And its candidate runs ran only
after the merge, from 13:01 to 13:07, while the resume polled for their scorecard (35 polls, 30 s
apart, `11-turn-resume-after-merge.log`). In trials 3 and 4 the candidate runs overlapped the
baseline runs.

## What each validator recorded

The same six records in trials 2, 3 and 4 (`evidence/` of each case; producer, then result):

| record | producer | result | trial 2 | trial 3 | trial 4 |
|---|---|---|---|---|---|
| 1 | `takt.finding-file` | `finding_record` filed | 12:49:46 | 13:37:46 | 14:36:40 |
| 2 | `takt.scenario-reproduce` | `scenario_result` reproduced, at the baseline | 12:50:12 | 13:38:23 | 14:37:13 |
| 3 | `takt.scenario-verify` | `scenario_result` not_reproduced, at the candidate | 12:50:45 | 13:39:05 | 14:37:54 |
| 4 | `takt.candidate-merge-judge-surface` | `merge_observation` merged: "main (forge: none) at `<candidate>` holds the candidate" | 12:51:00 | 13:39:20 | 14:38:22 |
| 5 | `takt.scorecard-compare` | `scorecard_result` no_regression, 0 scenarios worse | 13:08:06 | 13:40:50 | 14:39:53 |
| 6 | `takt.binary-promote` | `promotion_record` promoted | 13:08:56 | 13:42:00 | 14:40:45 |

The turn's own steps cost no tokens: it ran with `--harness none`, and every stop line says
`tokens 0`.

## How a turn ran

The commands, in the order of trial 4's `timeline.tsv`:

1. **Baseline.** Five scenario 1 runs with the trial's build, `takt scorecard --save`, then
   `takt promote --baseline-only --install-root <root>` (rows `07`, `08`).
2. **Candidate.** `takt run` on `story.delivery/1` delivered the fix in a clone of takt, with
   Claude Code as the harness. With no forge to open a pull request on, the delivery stopped on
   `NoProgress` (exit 3), and its branch head became the candidate (`notes.tsv`). Five scenario 1
   runs of the candidate's build followed.
3. **Open.** `takt run --protocol self.improvement/1 --install-root <root> --input finding=<story>
   --input candidate=<sha> --input surface=judge --scenarios <dir> --harness none`. Three steps:
   filed, reproduced, not reproduced. Then it stops: `ApprovalRequired
   (candidate.merge_judge_surface)`, exit 4.
4. **Approve and merge.** `takt approve <case> repository.merge.judge_surface --by human:<id>`,
   then the fast-forward of `main` to the candidate.
5. **Resume.** `takt run --case <case>`: merged, then `scorecard.compare` waits for the
   candidate's saved scorecard and records no_regression. It stops with `ApprovalRequired
   (binary.promote)`, exit 4.
6. **Promote.** `takt approve <case> takt.promote --by human:<id>`, then `takt promote` from a
   clean clone at the candidate. That is a confined release build, installed. The binary it
   replaced is kept at `$XDG_STATE_HOME/takt/judges/<sha256>`.
7. **Finish.** The last resume runs with that kept binary, the one that opened the turn:
   `binary.promote` records promoted, and the run stops `Completed (accepted)`.

Trial 2 ran before the turn recorded its judge binary, so its final resume ran with the trial's
build. Trial 3 set `CARGO_INSTALL_ROOT` on every turn run, since `takt run` had no
`--install-root` then (`notes.tsv`).

## The candidate, delivered by takt

| | trial 2 | trial 3 | trial 4 |
|---|---|---|---|
| delivery (`04-deliver-story-delivery`) | 123 s, 10 steps, 268,051 tokens | 171 s, 18 steps, 491,390 tokens | 111 s, 10 steps, 285,325 tokens |
| stop | `NoProgress`, exit 3 | the same | the same |
| promotion (`promote-candidate` row) | 37 s, sha256 `5317c0d3…` | 50 s, sha256 `92188fba…` | 38 s, sha256 `02fb338b…` |

Tokens are the delivery's stop line. They equal the sum of the delivery's harness transcripts'
`TOTAL` line (input + output + cache read + cache write).

## The scorecard

Five scenario 1 runs a side, each in a local bare origin with no forge and up to two resumes
(`run-scen.sh`). All 30 runs stopped on `NoProgress` after their two resumes, and all 30 passed
the held-out check (`acceptance=PASS` in each `scen-*` row). Tokens are from the `takt promote`
log (`saved` and `candidate` lines):

| | baseline median (min to max) | candidate median (min to max) | change | verdict |
|---|---|---|---|---|
| trial 2 | 205,558 (204,634 to 206,003) | 205,918 (205,499 to 615,395) | +360 | no scenario worse than the baseline |
| trial 3 | 221,639 (205,334 to 221,910) | 205,785 (205,215 to 222,223) | -15,854 | the same |
| trial 4 | 205,994 (205,853 to 221,822) | 205,790 (205,730 to 280,739) | -204 | the same |

Trial 2's 615,395 is one run, `scenario-1-trial-25`: 38 steps over three invocations, the third
of them 21 steps (`13-scorecard-save-cand.log`). These runs stop at a different place from the
scenario 1 runs in `docs/metrics/scenario-runs.jsonl`, which stop at the merge approval, so their
tokens do not compare with those. They have no line there.

## Tokens and cost per trial

The sum of the `TOTAL` lines of each trial's `logs/transcripts-*-analysis.txt`: one delivery and
ten scenario runs per trial, every harness call of the trial.

| | analyses | tokens | cost |
|---|---|---|---|
| trial 2 | 11 | 2,750,162 | $5.60 |
| trial 2, the aborted first baseline runs | 4 | 239,745 | $0.44 |
| trial 3 | 11 | 2,628,726 | $5.20 |
| trial 4 | 11 | 2,450,830 | $4.95 |

The trial's own `analyze-transcripts.py` produced trial 2's aborted row. It read the four
directories under `transcripts/aborted-same-case-id/` when this record was written, and no
analysis file holds it. With that row, trial 2 used 2,989,907 tokens.

## The probes

Each probe ran on a copy of the case and state taken just before the step it probes (`notes.tsv`):

| probe | what was done | result | log |
|---|---|---|---|
| trial 3, P1 | resume with the newly promoted binary | refused, exit 1, nothing run: the binary hashes to another sha256 than the one that opened the turn | `19-probes.log` |
| trial 3, P2 | resume with the kept judge binary and no install root set | polled 20 times in 611 s, killed; led to `--install-root` | `notes.tsv`, `19-probes.log` |
| trial 4, 1a | grant `--by agent:developer`, no `.engineering/delegations.yaml` at `main` | refused at the merge step, exit 3, 0 merge records | `19a-probe-delegation-absent.log` |
| trial 4, 1b | the same, `delegations: []` at `main` | refused, exit 3, 0 merge records | `19b-probe-delegation-empty.log` |
| trial 4, P2a, P2b | resume with `--install-root`, another root and then the same root | refused, exit 1, "nothing was resumed"; `events.jsonl` 12 lines before and after | `19c-probe-install-root.log` |
| trial 4, P1 | resume with the newly promoted binary | refused, exit 1, in 62 ms | `19d-probes-after-accept.log` |
| trial 4, P2 | resume with the kept judge and `CARGO_INSTALL_ROOT` pointing elsewhere | `Completed (accepted)`: the case's recorded install root was read | `19d-probes-after-accept.log` |

## Deviations

1. Scratch state, a scratch install root, `--forge none` and the operator's fast-forward of
   `main`, as above.
2. Trial 2's `takt approve` also recorded the approval in the scratch planning store
   (`09-approve-judge-surface.log`). From part 2 correction 1, a turn's grant goes into the case's
   `authority.yaml` only.
3. Each install root was seeded with the trial's own build before `promote --baseline-only`,
   which refuses when no binary is installed (`notes.tsv`, trials 3 and 4).
4. In each trial a scratch commit moved the finding from draft to proposed, so that
   `story.delivery/1` would take it (`03-setup-clones.log`, `notes.tsv`).
5. The candidate came from a delivery that stopped on `NoProgress`, not from a merged pull request.

## Code

RSI part 1 (opening a turn, `declined`, `unreproducible`) shipped in 0.2.0 and 0.2.1. RSI part
2 holds the merge validators, `binary.promote`, the judge binary recorded at open,
`--install-root`, and the judge-surface rule it shares with the judge guard. It is on
`impl/rsi-part-2`, tip `0b64432d`, and ships as 0.2.2. That tip adds one commit after trial 4: the
install root is recorded canonical, and a missing root is refused at open.

Sources: the trial records named above; `git log fe89e2c..0b64432d`.
