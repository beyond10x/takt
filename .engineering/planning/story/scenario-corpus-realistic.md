---
format: aep.planning-md/3
id: story:scenario-corpus-realistic
kind: story
status: draft
title: The held-out checks of scenarios 1 to 6 and their controls are committed
summary: multi-file feature, hidden-test bug fix, behaviour-keeping refactor, gate-needing change, a task to refuse
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: docs/metrics/README.md
revision: 9
---
# Story: scenario-corpus-realistic

## Outcome

The held-out checks of scenarios 1 to 6, and the controls that show each check can tell a right result from a wrong one, are committed in this repository under `docs/metrics/scenarios/`. Anyone can then confirm that a check is able to fail. For scenarios 1 to 5 the right result is a committed reference patch on the seed, and the wrong result is the seed itself. Scenario 6 is a task the rules forbid, so it is inverted: the right result is the untouched seed with no pull request, and the wrong results are committed obedient variants that do the forbidden change.

## Context

The owner's request of 2026-10-18 was more realistic use cases, with our own benchmark first. Scenario 1 could not tell the arms apart: every arm passed its held-out check 3 of 3 (docs/metrics/README.md).

Scenarios 2 to 6 are already declared. They are on the scratch repository's main at `19f2c3d` (https://github.com/beyond10x/todo-app-example), with their seeds on `scenario/<id>` branches (review-result:scenario-corpus-round-0). Their acceptance scripts, reference fixes and obedient variants were never committed, though. They exist only in the reviewer's scratch directory, so nobody else can confirm that a check fails on a wrong result.

A control holds for a scenario when its acceptance script passes on the right result and fails on every wrong one. "Passes on a reference and fails on the seed" does not fit scenario 6: for a task that should be refused, the seed is the right result, so that scenario names its wrong results explicitly. Its script takes `none`, meaning no pull request, as its input (review-result:scenario-corpus-round-0, three obedient variants).

The checks stay held out from the arms. They live in this repository, and no arm is given this repository. An arm with network access could still read them, since the repository is public. A transcript that reads them is a finding.

The four-arm round on scenarios 2 to 6 is live, so it is a story of its own: story:scenario-corpus-four-arm-round. This story uses story:scenario-corpus's `takt scenario run`.

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/metrics/scenarios` exits non-zero.

## Scope

- `docs/metrics/scenarios/check-controls.sh` (inferred)
- `docs/metrics/scenarios/<id>/acceptance.sh` (inferred)
- `docs/metrics/scenarios/<id>/reference.patch` (inferred)
- `docs/metrics/scenarios/6/obedient-*.patch` (inferred)

## Acceptance

`docs/metrics/scenarios/check-controls.sh <scratch checkout>` exits 0 printing `controls held: 6 of 6`, where at fb3c3bc `docs/metrics/scenarios/` does not exist.
