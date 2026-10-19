---
format: aep.planning-md/3
id: story:improvement-turns-2-and-3-recorded
kind: story
status: draft
title: Improvement turns 2 and 3 are recorded as accepted cases
relations:
- decomposes: epic:recursive-self-improvement
- depends_on: story:case-id-is-a-valid-branch-name
- depends_on: story:hours-bounds-a-harness-step
- depends_on: story:promotion
- serves: vision:safe-agentic-coding
revision: 2
---
## Outcome

Improvement turns 2 and 3 are recorded as `self.improvement/1` cases under `.engineering/cases/`, each with every claim held on evidence: finding filed, reproduced at the promoted baseline, not reproduced at the candidate, no regression at n=5 under the decided promotion rule, merged under the planner's grant, promoted; and `docs/metrics/improvement-turns.jsonl` has a line for each. With turn 1 that makes the three recorded turns story:loop-scorecard's acceptance needs.

## Context

Owner: the planner, recording in phase R0 with the turn-1 recorder, as turn 1 was (docs/runs/2026-10-18-improvement-turn-1/). The plan-reviewer audit of 2026-10-18 found that no artifact owned these records.
- Turn 2: story:case-id-is-a-valid-branch-name. Case `turn-2-case-id` holds e1-e4 and the merge grant at 834205f (the planner's case tree); the candidate runs T-24..28 at 834205f against the baseline T-14..18 at a0b8576 gave no regression (median 122,465 against the baseline maximum 138,160). Merged into wave 12. Open: the merge into main and a promotion.
- Turn 3: story:hours-bounds-a-harness-step. `scripts/check-finding` reproduced it at a0b8576 and did not at cfbf7e4, the revision its binary embeds; the candidate runs T-39..43 at cfbf7e4 reached rank 2 in 5 of 5, token median 122,151 against the baseline maximum 138,160: no regression. Granted at cfbf7e4 and merged into wave 12 at 3b0b748. Open: the case record (e1-e5 and the measurement), the merge into main and a promotion.
- One promotion a UTC day (story:promotion): turns 2 and 3 share one promotion, the first after wave 12 reaches main, since a promotion of a revision holding both candidates serves both turns' `promoted` claim. Turn 1 was promoted 2026-10-18T16:02Z.

## Scope

- `.engineering/cases/turn-2-case-id/`, `.engineering/cases/turn-3-<key>/` (new, copied from the recorder's case tree)
- `docs/runs/2026-10-18-improvement-turns-2-3.md`, `docs/metrics/improvement-turns.jsonl` (the doc-writer)

## Acceptance

Both cases end `accepted` at their last Canon decision with all six claims true, each claim's evidence named in the case notes; `improvement-turns.jsonl` has three lines; `takt scorecard` (or `loop.measure` once it lands) reads three `self.improvement` cases under `.engineering/cases/`.
