# Run record: the first turn promoted by the installed takt, 2026-10-19

One `self.improvement/1` turn, `takt-20261019T180323Z`, ran in the installed takt's own state:
its real promotion store and its real install root. The self-improvement trials
(`docs/runs/2026-10-19-self-improvement-trials/`) ran in scratch ones. The turn reached
`accepted` on six records, each written by a validator, and the installed takt promoted the
candidate.

The run's records stay outside the repository because they hold local paths: `logs/timeline.tsv`
(one row per command: start time, name, exit code, seconds, command), `logs/notes.tsv`, one log
per command, the turn's case directory, and the promotion record
`7ae5b05b63971efa76c1e8bb9acfc1e8f390b56e.json` in the installed takt's promotion store. Every
figure below is read from them, and the file is named beside it. Times are UTC.

**Commit ids.** The case, the promotion record and the installed binary's `--version` name the
commits as they were when the turn ran. Afterwards `main`'s history was rewritten so that every
commit is by the bot identity, with the trees unchanged (`git rev-parse <sha>^{tree}`). On `main`
the candidate `7ae5b05b` is `262bfa32`, its merge `37b3f2d0` is `ceb22fe0`, and the finding's
revision `5b19b9eb` is `41fed5e2`. Below, a commit keeps the id its record names.

## The turn

| | |
|---|---|
| judged by | takt 0.2.3 at `78c1a08e`, installed by a promotion at 17:57 (`notes.tsv`, 17:57:39); each of the four `opened` events in the case's `events.jsonl` names binary `78c1a08e` |
| finding | `story:scorecard-help-names-the-held-out-check` at `5b19b9eb` (`events.jsonl`, line 3) |
| candidate | `7ae5b05b`, on `main` as `262bfa32` through the merge `ceb22fe0` |
| opened to accepted | 18:03:23 to 18:20:59, 17 min 36 s: the case's `opened` event (line 1) to its stop `Completed (accepted)` (line 32) |
| outcome | `accepted`, decision `0011.json`, `computed_from` 18:20:59 |
| grants | two, both by `human:owner`: `repository.merge.judge_surface` at 18:05:26 and `takt.promote` at 18:20:14 (`authority.yaml`) |
| promotion | a record of `kind` `promotion`, `against` `78c1a08e`, `worse` 0, rule 5 trials and margin 1 (the promotion record) |

## What each validator recorded

From the case's `evidence/0001` to `0006`; every step of the turn is `by` `validator` in
`events.jsonl`.

| step | record | result | observed |
|---|---|---|---|
| `finding.file` | `finding_record` | `filed`, at `5b19b9eb` | 18:03:23 |
| `scenario.reproduce` | `scenario_result` | `reproduced` at `78c1a08e` | 18:04:02 |
| `scenario.verify` | `scenario_result` | `not_reproduced` at `7ae5b05b` | 18:05:20 |
| `candidate.merge_judge_surface` | `merge_observation` | `merged` | 18:05:29 |
| `scorecard.compare` | `scorecard_result` | `no_regression` | 18:20:09 |
| `binary.promote` | `promotion_record` | `promoted` | 18:20:59 |

## How the turn ran

| start | row of `timeline.tsv` | exit | what happened (its log) |
|---|---|---|---|
| 18:03:21 | `11-turn-open` | 4 | three steps, stop `ApprovalRequired (candidate.merge_judge_surface)` |
| 18:05:26 | `12-approve-judge-surface` | 0 | the judge-surface merge granted |
| 18:05:26 | `13-turn-resume-after-merge` | 6 | merge observed; `scorecard.compare` waited for the candidate's saved scorecard |
| 18:05:43 | `gh-T-6` to `gh-T-10` | 4 each | five scenario-1 runs of the candidate, each to the merge approval, held-out check pass |
| 18:20:06 | `14-scorecard-save-cand` | 0 | the candidate's scorecard saved: n=5, median tokens 66,421 against the baseline's saved maximum 75,992 (baseline n=5, median 73,739) |
| 18:20:09 | `15-turn-resume-compare` | 4 | `no_regression`; stop `ApprovalRequired (binary.promote)` |
| 18:20:14 | `16-approve-takt-promote` | 0 | the promote granted |
| 18:20:14 | `17-promote-candidate` | 0 | the installed takt's `takt promote`: "no scenario worse than the baseline"; it built and installed `7ae5b05b` and kept the binary it replaced as the judge |
| 18:20:59 | `18-turn-resume-final-kept-judge` | 0 | under the kept judge: `binary.promote` `promoted`, stop `Completed (accepted)` |

## The candidate

A `story.delivery/1` run of the installed takt 0.2.3 wrote it (`09-deliver-story-delivery.log`,
`--forge none`, `--model claude-opus-5-5`): 10 steps, 166,753 tokens, 113 s, stopped on
`NoProgress`, exit 3. Its two commits change the help text of `takt scorecard` and add a
`scorecard_cli` test. The operator cherry-picked them onto `67773029` as `7ae5b05b`, where
`scorecard_cli` passed 13 of 13 (`notes.tsv`, 18:01:36).

## Deviations

- The delivery run did not finish: it stopped on `NoProgress`, and the operator moved its commits
  onto the turn's base by hand.
- The candidate reached `main` before the turn opened: the merge `ceb22fe0` (`37b3f2d0` before
  the rewrite) is dated 18:02:18 (its committer date), and the judge-surface grant came at
  18:05:26. The validator observed a merge that was already there.
- The resumes took no `--forge` flag and read the forge as `bot`: the merge step only observed
  (`notes.tsv`, 18:05:38).
- The operator's runner, not the turn, ran the five candidate runs and saved their scorecard.
- The `11-turn-open` row starts at 18:03:21, two seconds before the case's `opened` event. Measured
  as in the trials' record, from the row's start to the final `computed_from`, the turn took
  17 min 38 s.
- The ten scenario runs of this run (`gh-T-1` to `gh-T-10`) have no line in
  `docs/metrics/scenario-runs.jsonl`, and the turn has none in `improvement-turns.jsonl`.
