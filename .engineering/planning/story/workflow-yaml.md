---
format: aep.planning-md/3
id: story:workflow-yaml
kind: story
status: archived
title: 'M3: workflows as workflow/1 YAML'
summary: protocol, step, loop, gate and approval blocks; a repository default; a three-iteration outer loop
relations:
- decomposes: epic:composable-agentic-work
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T01:43:14Z", actor: "human:timo", revision: 3}
---
## What

`workflow/1`, a YAML document of blocks: `protocol` (id, revision, inputs), `step` (deterministic:
action and argv; judgement: actions and brief), `loop` (steps and an `until`: a claim, an outcome
or a budget), `gate` (a `canon-expr/1` assertion and its provider) and `approval` (capability and
approver). A repository default lives in `.engineering/workflow.yaml`; `takt run --workflow <file>`
runs one.

## Acceptance

The M2 story runs from a `workflow/1` file with an inner loop until `tests.pass`, a validation loop
until the gate is green and an outer loop of three iterations over three ready stories, and the
decision documents equal those of the same run without the file.
