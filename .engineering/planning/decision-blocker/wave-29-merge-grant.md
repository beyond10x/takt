---
format: aep.planning-md/3
id: decision-blocker:wave-29-merge-grant
kind: decision-blocker
status: cleared
title: Merge the whole wave 29 diff (judge surfaces) into main on a clean verdict at its sha
relations:
- blocks: story:file-finding-resolves-paths-without-gnu-realpath
- blocks: story:darwin-job-has-flock
- blocks: story:takt-runs-its-tests-on-macos
- blocks: story:validators-perform-as-effects
- blocks: story:confinement-seam
- blocks: story:replay-offers-granted-actions
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-21T18:52:33Z", actor: "human:timo", revision: 2}
---
## Question

May the whole wave 29 diff, `main..<merge sha>`, merge into `main`? Every unit but the wave page touches a judge surface, and `.engineering/delegations.yaml` holds `delegations: []`. decision-blocker:governor-judge-surface-merge covers story:governor-holds-takts-cases only.

| option | what it does |
|---|---|
| A | one grant for the whole diff `main..<merge sha>`, on the reviewer's clean verdict at that exact sha |
| B | a delegation in `.engineering/delegations.yaml` naming the planner |

## Answer

Option A, decided by the owner's coordinator on the owner's instruction of 2026-10-21 18:51Z, verbatim: "you should not prompt me with decissions like the one above, I expect you to decide for me in which order or batch size to process waves/stories". "The one above" was this question.

- The grant holds only on a clean reviewer verdict at the exact merge sha. A finding goes back to the coordinator before the merge.
- Option B is not taken. A delegation record in `.engineering/delegations.yaml` stays the owner's to make.
- Later waves get the same: one grant per merge, from the coordinator, on a clean verdict at its sha.
