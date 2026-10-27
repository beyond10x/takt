---
format: aep.planning-md/3
id: story:story-acceptance-scenarios-run-in-task-check
kind: story
status: draft
title: The conformance scenarios a story's acceptance names are listed in ess-inputs and run by task check
summary: ess/ess-inputs.yaml lists no scenario although story acceptance names them
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

Every scenario a takt story's acceptance names is a conformance scenario listed in
`ess/ess-inputs.yaml` and run by `task check`, so a story's acceptance can be read off the
specification's own suite.

## Context

- Found by the adversary pass on the fresh-repository wave (finding 6, pre-existing): the
  acceptance of story:gate-commands-find-programs-on-a-system-path and
  story:harness-none-takes-a-granted-action says "Each item is a conformance scenario named in
  `ess/ess-inputs.yaml`, run by `task check`", and `ess/ess-inputs.yaml:6` holds `scenarios: []`
  since story:takt-domain-model created the file. Other stories carry the same sentence.
- Today each acceptance item is a cargo test named after the scenario id; no scenario is listed or
  run by the specification's suite.

## Scope

- `ess/ess-inputs.yaml`: the scenarios (cited)
- `Taskfile.yml`: `task check` runs the conformance suite (inferred)
- `ess/domains/takt.yaml`: the scenarios' commands and views (inferred)

## Acceptance

- `ess/ess-inputs.yaml` lists at least the scenarios of the implemented stories whose acceptance
  names them, and `task check` fails when one of them fails.
- A story whose acceptance names a scenario the file does not list is reported by `task check`.
