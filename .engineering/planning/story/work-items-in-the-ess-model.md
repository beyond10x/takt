---
format: aep.planning-md/3
id: story:work-items-in-the-ess-model
kind: story
status: draft
title: The ESS model declares work sources, keys and items
summary: Source, Key and Item in takt.work, Unit widened, before code builds on them
relations:
- decomposes: epic:work-sources
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: ess/domains/takt.yaml
revision: 2
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
