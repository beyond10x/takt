---
format: aep.planning-md/3
id: approval-record:wave-49-four-stories
kind: approval-record
status: approved
title: 'Wave 49: tests first, the CI wait, the token rule and the docs site'
summary: one wave over the active stories; the CI suite story is already implemented
relations:
- decides: story:story-delivery-tests-first
- decides: story:ci-wait-watches-the-check
- decides: story:the-token-rule-holds-for-an-unchanged-binary
- decides: story:docs-site-with-sidebar
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-03T13:02:16Z", actor: "human:timo", revision: 2}
---
# Approval: wave 49

## Decision

The owner, 2026-11-03, through the orchestrator: one takt wave of at most four units over the
active stories story:story-delivery-tests-first, story:ci-wait-watches-the-check,
story:the-token-rule-holds-for-an-unchanged-binary and story:the-cli-suite-fits-its-ci-budget,
with story:docs-site-with-sidebar only if a unit is left. story:harness-child-confined stays out
(blocked: dependency). No trial is run or started by the wave.

## Selection, read at origin/main 97c56193

`aep plan artifact waves --status active` (aep 0.79.0), verbatim:

```
wave 1
  story:docs-site-with-sidebar (inferred)
  story:story-delivery-tests-first (inferred)
wave 2
  story:ci-wait-watches-the-check (inferred)
collision: story:ci-wait-watches-the-check story:story-delivery-tests-first protocols/story-delivery/2.yaml (inferred)
unassessed: story:harness-child-confined
unassessed: story:the-cli-suite-fits-its-ci-budget
unassessed: story:the-token-rule-holds-for-an-unchanged-binary
2 wave(s), 1 collision(s), 3 unassessed
```

- story:the-cli-suite-fits-its-ci-budget is `implemented` in the store (transition of
  2026-10-29T21:30:01Z) and leaves the wave; nothing is done for it.
- story:ci-wait-watches-the-check and story:story-delivery-tests-first collide on the protocol
  document (in this tree `protocols/story-delivery/3.yaml`); they run one after the other, the
  CI wait after the tests-first unit merges.
- story:docs-site-with-sidebar: a unit is left, so it is in. Its site landed at 5d8adae0
  (2026-10-19); the wave checks the acceptance against the tree and records it.

## Commits this approval authorises

One commit or more per unit on its unit branch, the merges into `wave/49`, the store commits
(this record, evidence, moves, scope), the release prepare, one pull request through the bot
route, and its merge to `main` on green CI. Tag and release follow the owner's go.
