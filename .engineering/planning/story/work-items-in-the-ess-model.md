---
format: aep.planning-md/3
id: story:work-items-in-the-ess-model
kind: story
status: implemented
title: The ESS model declares work sources, keys and items
summary: Source, Key and Item in takt.work, Unit widened, before code builds on them
relations:
- decomposes: epic:work-sources
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: ess/domains/takt.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T13:11:50Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-06T13:11:51Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-06T20:55:01Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Story: work-items-in-the-ess-model

## Outcome

takt's ESS model (`ess/domains/takt.yaml`, domain `takt.work`) declares where a run's work comes from before code builds
on it: a work source (its name, one of `aep`, `jira`, `github`, `gitlab`, `command`, and an optional connection), a
work key (`<source>[@<connection>]:<id>`), and a work item (key, title, body, url, revision), and widens `Unit` and the
case so a case holds a work item, a story being the `aep` one.

## Context

`Unit` is declared as intent or story (`ess/domains/takt.yaml`, `Unit`); `Story` holds `status`, `open_case` and
`approvals`. The domain is already named `takt.work`, so the new types are `takt.work.Source`, `takt.work.Key` and
`takt.work.Item` rather than a second domain.

## Acceptance

- `ess specify validate --path ess` exits 0 with the three types and the widened `Unit` declared, each `read from:`
  citing the code line that says it, or `inferred` where no code exists yet.
- The key grammar is declared with examples: `aep:story:x`, `jira:ABC-12`, `jira@team-a:ABC-12`,
  `github:owner/repo#42`, `gitlab:group/project#7`.
- The citations check passes with digests.

## Scope

- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv`

## Status

Built in wave 56 (unit a, 3d16bb309), released in 0.34.0 from 2c4f1d7f4, the merge of pull request 63.

- `ess/domains/takt.yaml`: `takt.work.SourceName`, `Source`, `Key`, `Item` and `WorkCommands`, the run file's `work`
  and `work_commands`, the case's `item_key` and `item_revision`, and the tracker case-id scheme
  (approval-record:wave-56-work-model-points). The declarations cite the code since a19b058f2.

Review: review-result:w56-adv-1.
