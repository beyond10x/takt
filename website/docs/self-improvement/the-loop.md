---
title: The loop
description: "How takt improves takt: three levels, one improvement turn, how an operator runs one, and the phases from hand records to takt driving the turn."
---

# The loop

| level | what is improved | protocol | judged by |
|---|---|---|---|
| L0 work | a repository's stories | `story.delivery/1` | Canon over validator evidence |
| L1 improve takt | takt's code, protocols and validators | `self.improvement/1` | the [scorecard](./scorecard.md): no regression against the promoted revision |
| L2 improve the loop | the scenario corpus, the scorecard's metrics, the review panel, the intake rules, the improvement protocol, the agent profiles | `loop.improvement/1` | the loop scorecard |

## One improvement turn

run → record → measure → diagnose → file → deliver → verify → promote.

- **Run and record.** The reviewer runs scenarios; each recorded run is a line in
  `docs/metrics/scenario-runs.jsonl` ([Runs](./runs.mdx)).
- **File.** A finding carries an `expected_before` predicate a machine can check: a command, run on
  a named scenario, with its expected exit status or output. Without one, it is a note. It names
  the run record or review that produced it.
- **Deliver.** A candidate fix is delivered outside the turn: by the developer in a wave, or by
  takt on `story.delivery/1`.
- **Verify.** A fix is verified when the predicate held at the promoted revision and fails at the
  candidate (`scripts/check-finding`), checked by a run other than the one that delivered the fix.
  A candidate at which the predicate still reproduces is declined.
- **Promote.** `takt promote` installs the candidate only when its scorecard shows no scenario worse
  than the promoted baseline.

## What a turn can reach

A `self.improvement/1` turn ends in one of four outcomes:

| outcome | when |
|---|---|
| `unreproducible` | the finding's predicate does not hold at the baseline |
| `declined` | the predicate still holds at the candidate |
| `accepted` | filed, reproduced, not reproduced at the candidate, merged into the mainline, no regression on the scorecard, promoted: each recorded by a validator |
| `abandoned` | the case is abandoned explicitly |

0.2.0 and 0.2.1 can end a turn `unreproducible` or `declined`, never `accepted`: they have no
merge validators and no `binary.promote`. 0.2.2 adds them (`story:self-improvement-protocol`,
RSI part 2). In the self-improvement trials of 2026-10-19, takt drove three turns to `accepted`
that way, in scratch state ([Turns](./turns.md)).

## Running a turn

The order the trials ran it in. `<root>` is the install root, where `takt promote` installs takt.

1. **Baseline.** The promotion store's latest record is the baseline, with the scorecard it was
   promoted on. The installed binary is the one that opens the turn. A store with no record yet
   takes one from `takt promote --baseline-only`, as the trials did.
2. **Deliver.** The candidate is a commit on a branch: a developer's fix, or the head of a
   `story.delivery/1` run on the finding. The candidate's scenario runs give its scorecard,
   saved with `takt scorecard --save`.
3. **Open.**

   ```sh
   takt run --repo <takt checkout> --protocol self.improvement/1 --install-root <root> \
     --input finding=<story id> --input candidate=<full commit id> --scenarios <scenario checkout> \
     --harness none
   ```

   The turn records the sha256 of the binary that runs it, and its install root. Then three
   steps run: filed, reproduced at the baseline, not reproduced at the candidate. The run stops
   on `ApprovalRequired (candidate.merge_judge_surface)`, exit 4. For a candidate that touches no
   judge surface the step is `candidate.merge` instead, under a `repository.merge` grant.
4. **Approve the merge, then merge.** `takt approve --repo <takt checkout> <case>
   repository.merge.judge_surface --by human:<id>`. The grant is recorded in the case only. Then
   merge the candidate into `main`: through the forge, or, under `--forge none`, into the local
   `main`, which the operator is trusted with. An `agent:<role>` grant counts only when
   `.engineering/delegations.yaml` at the mainline tip delegates that capability to the role.
5. **Resume.** `takt run --repo <takt checkout> --case <case>` records the merge. It then
   compares the candidate's saved scorecard with the baseline's and stops on `ApprovalRequired
   (binary.promote)`.
6. **Promote.** `takt approve --repo <takt checkout> <case> takt.promote --by human:<id>`, then
   `takt promote --repo <clean clone at the candidate> --scorecard <candidate scorecard>
   --install-root <root>`. It installs the candidate and keeps the binary it replaced at
   `$XDG_STATE_HOME/takt/judges/<sha256>`, and prints that path.
7. **Finish with the kept binary.** The turn refuses every resume by another binary, the newly
   promoted one included. Check `sha256sum` of the kept binary against the name it is kept
   under. Then run it: `$XDG_STATE_HOME/takt/judges/<sha256> run --repo <takt checkout> --case
   <case>`. `binary.promote` records promoted, and the run stops `Completed (accepted)`.

In `self.improvement/1`, a judge-surface candidate needs `repository.merge.judge_surface`
([Evidence and the judge](../concepts/evidence-and-the-judge.md#the-judge-for-takts-own-changes)).

## Phases

| phase | who delivers takt's stories | state |
|---|---|---|
| R0 | the developer agent in waves; the planner plans and grants, the reviewer reviews and runs trials, the doc-writer keeps the docs | now |
| R1 | takt runs the scratch repository's scenarios daily | entry: scenario 1 accepted unattended, one invocation plus one grant, daily for a week |
| R2 | takt delivers takt's non-judge stories under the pinned judge | entry: the hardening stories done (wave 06) and a scorecard baseline recorded |
| R3 | takt proposes stories from its scorecards; the owner prioritises | entry: two weeks of R2 with no regression |

In R0, turns 1 to 3 were performed by the agents, who recorded the evidence by hand. From 0.2.2,
validators record the evidence of an L1 turn. So far that has happened only in the trials, in
scratch state; the operator's installed takt has not yet promoted a revision through a turn.

Sources: `design:recursive-self-improvement`, `protocols/self-improvement/1.yaml`,
`takt run --help`, `takt promote --help`, `docs/runs/2026-10-19-self-improvement-trials/README.md`,
`docs/design/takt.md` § 6.
