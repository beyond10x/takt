---
format: aep.planning-md/3
id: story:run-swimlane
kind: story
status: draft
title: 'Each run has a swimlane image: actor lanes, token bars, on its run page'
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:run-trace-and-report
- depends_on: story:docs-site-with-sidebar
revision: 1
---
## Outcome

`takt-docs generate` draws, for every committed trace, `runs/<run>.svg`: x is seconds from the run's start; one lane per actor (harness, validator, forge, takt, idle, human); a model call is a block whose height is its tokens in four stacked components, hollow when it changed nothing; validator operations are thin and coloured by outcome; fixed sleeps are hatched; tool calls inside a step are a sub-row of the harness lane; pattern labels are drawn; a footer gives the coverage ("18 of 18 ops timed, 3 of 3 calls with usage, cost: not recorded"). The image is inline on the run's page, with the run report beside it, and the site's runs index links every run.

## Context

From the owner's ask of 2026-10-19 (several reporters over one trace, the journey image on the public site, a report per run, more self-improvement on the site, the beyond10x docs style with a sidebar) and the read-only design written for it the same day. `docs/metrics/journey.py` draws one lane per run with no tokens or cost (`journey.py:10`, drawing `:594-757`), for scenario 1 only (`:761`), and its image is not on the site. This replaces its drawing; `journey.py` retires when the crate's output covers it, checked in this unit's test.

## Scope

- `crates/takt-docs/`: the swimlane renderer and the run pages (inferred)
- `website/docs/runs/` (generated) and `website/static/metrics/` (inferred)
- `docs/metrics/journey.py`: retired (cited)

## Acceptance

For the T-24 and trial-1 traces, the generated SVG has the six actor lanes, one block per model call whose height ratio between two calls equals their token ratio, and a coverage footer whose counts equal the trace's; the run page shows the image and the report; `generate --check` fails when the SVG is edited by hand.
