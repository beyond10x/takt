---
format: aep.planning-md/3
id: story:scenario-corpus-four-arm-round
kind: story
status: draft
title: Scenarios 2 to 6 each run once under each of the four arms, with the held-out verdict recorded
relations:
- informed_by: review-result:benchmark-acceptance-round-1
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:scenario-corpus-realistic
- depends_on: story:baseline-runner-in-repo
revision: 1
---
## Outcome

`docs/metrics/scenario-runs.jsonl` holds a row with a held-out verdict for every pair of a scenario from 2 to 6 and an arm of the baseline method: `takt`, `claude-code`, `claude-code-isolated` and `codex`. Each realistic scenario is then a comparison, not a takt-only reading.

## Context

This was split from story:scenario-corpus-realistic, whose earlier acceptance joined the offline controls with this live round. Today only the takt arm has run on scenarios 2 to 6, once each at a0b8576 (docs/metrics/README.md § Scenarios 2 to 6; review-result:scenario-corpus-round-0). That is 5 of the 20 pairs.

The round is live: four harnesses, a model, the scratch repository's forge. The acceptance only reads the committed records, so it can be checked offline. The runs use the runner in the repository (story:baseline-runner-in-repo) and the committed checks (story:scenario-corpus-realistic). `docs/metrics/README.md` § `scenario-runs.jsonl` gives the row fields. One run per pair is a pilot, not a result. How many runs a claim needs is set by the scorecard rule (design:recursive-self-improvement § Scorecard).

## Expected before

On takt at main, holds; after the change, fails:

- The rows of `docs/metrics/scenario-runs.jsonl` with `scenario` 2 to 6 and an `acceptance` of `pass` or `fail` cover 5 pairs of scenario and arm, all of them `takt`.

## Scope

- `docs/metrics/scenario-runs.jsonl` (inferred)
- `docs/metrics/README.md` (inferred)

## Acceptance

The rows of `docs/metrics/scenario-runs.jsonl` whose `scenario` is 2 to 6 and whose `acceptance` is `pass` or `fail` cover all 20 pairs of those scenarios and the arms `takt`, `claude-code`, `claude-code-isolated` and `codex`, where at d17bc32 they cover 5.
