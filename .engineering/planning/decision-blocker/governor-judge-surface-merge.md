---
format: aep.planning-md/3
id: decision-blocker:governor-judge-surface-merge
kind: decision-blocker
status: cleared
title: 'Owner: merge story:governor-holds-takts-cases (judge surfaces) into main at wave 29''s close'
relations:
- blocks: story:governor-holds-takts-cases
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-21T18:43:43Z", actor: "human:timo", revision: 2}
---
## Question

May story:governor-holds-takts-cases (32a0085) merge into `main` at wave 29's close? Its diff `42d8efa..32a0085` touches judge surfaces, `AGENTS.md` § Rules among them. `.engineering/delegations.yaml` holds `delegations: []` on `main` and on `wave/29`, so the grant is the owner's.

| option | what it does |
|---|---|
| A | grant the merge at wave 29's close, after the reviewer's verdict on the exact merge sha |
| B | hold the story out of wave 29's merge |

## Answer

Owner, 2026-10-21 18:42Z, verbatim: "A". The question named story:governor-holds-takts-cases and the merge at wave close.

Conditions, as given with the answer:

- The grant holds only if the reviewer's verdict at the merge sha is clean. Any finding goes back to the owner before the merge.
- The replay gap is fixed in wave 29, not deferred. `takt replay` rebuilds a step's offer from `decisions/NNNN.json` (`crates/takt/src/replay.rs:213-220`), which finds a granted action approval-required.

## Not covered

The answer covers this story's diff only. Wave 29's other units also touch judge surfaces (every path but `docs/**`, `README.md` and `CHANGELOG.md`): 22476ae, 015eee3, 42d8efa, 87893a5, the plan commits under `.engineering/planning/`, and the units still to come. Each needs its own owner grant before the merge.
