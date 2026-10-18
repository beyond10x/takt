---
format: aep.planning-md/3
id: story:merge-needs-the-storys-tests
kind: story
status: draft
title: A story's merge approval waits for the tests its acceptance names
relations:
- informed_by: review-result:scenario-corpus-round-0
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

`repository.merge` becomes approval-required only when the change carries the tests the story's acceptance names (or a recorded reason why not), not on CI and an open pull request alone.

## Context

From the first runs of the realistic scenario corpus (.engineering/planning/review-result/scenario-corpus-round-0.md): the `repository.merge` precondition checks only `ci.green` and `pr.opened` (`protocols/story-delivery/1.yaml:135`). In scenario 3 (a reported bug whose story's Acceptance names three new tests), takt requested the merge approval with `tests/test_todo.py` identical to the seed's. The held-out acceptance checks behaviour only, so it passed; the story's own acceptance was not met.

## Scope

- `protocols/story-delivery/1.yaml`: the `repository.merge` precondition and a claim about the story's tests (cited)
- `crates/takt/src/validator.rs`: the evidence that claim reads (inferred)

## Acceptance

On scenario 3, a run whose change leaves the seed's tests unchanged does not reach `ApprovalRequired (repository.merge)`; a run that adds the story's tests does.
