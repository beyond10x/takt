---
title: The loop
description: "How takt improves takt: three levels, one improvement turn, and the phases from hand records to takt driving the turn."
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
- **Deliver.** The developer fixes it in a wave.
- **Verify.** A fix is verified when the predicate held at the promoted revision and fails at the
  candidate (`scripts/check-finding`), checked by a run other than the one that delivered the fix.
  A candidate at which the predicate still reproduces is declined.
- **Promote.** `takt promote` installs the candidate only when its scorecard shows no scenario worse
  than the promoted baseline.

In `self.improvement/1`, a judge-surface candidate needs `repository.merge.judge_surface`
([Evidence and the judge](../concepts/evidence-and-the-judge.md#the-judge-for-takts-own-changes)).

## Phases

| phase | who delivers takt's stories | state |
|---|---|---|
| R0 | the developer agent in waves; the planner plans and grants, the reviewer reviews and runs trials, the doc-writer keeps the docs | now |
| R1 | takt runs the scratch repository's scenarios daily | entry: scenario 1 accepted unattended, one invocation plus one grant, daily for a week |
| R2 | takt delivers takt's non-judge stories under the pinned judge | entry: the hardening stories done (wave 06) and a scorecard baseline recorded |
| R3 | takt proposes stories from its scorecards; the owner prioritises | entry: two weeks of R2 with no regression |

In R0 the agents perform the turn's actions and record its evidence by hand. The validators that
would let takt drive an L1 turn are not built (`story:self-improvement-protocol`), nor is the judge
guard (`story:judge-guard`).

Sources: `design:recursive-self-improvement`, `protocols/self-improvement/1.yaml`,
`docs/design/takt.md` § 6.
