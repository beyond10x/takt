---
format: aep.planning-md/3
id: story:public-site-content
kind: story
status: implemented
title: The public site says enough to decide whether to try takt
summary: problem, how it works, status, measurements with n, timeline, roadmap and getting started on the static site
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:improvement-timeline-view
scope:
- confidence: cited
  path: gh-pages:index.html
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T14:38:04Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T14:38:04Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-18T14:38:05Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: public-site-content

## Outcome

The public site (https://beyond10x.github.io/takt/) says enough for a newcomer to decide whether to try takt: the problem it solves against a harness running free, how a run works (protocol, case, one step at a time, validators, grant), what is built and what is not (milestones with dates), the measured results with their n and caveats, the improvement timeline, the roadmap, and how to install and run it, with links into the repository's README and design. It stays a static page with no build framework.

## Context

Owner request of 2026-10-18, after the first version: "i wanted the pub docs minimal, it turned out to be a bit too minimal maybe". The first version (gh-pages 6a9171e, one 89-line `index.html`) has what takt is, five how-it-works bullets, a three-row status table, the usage block and links. It has no reason to prefer takt over a free-running harness, no figures, no timeline and no roadmap. The site is deployed from the orphan `gh-pages` branch so that publishing it pushes no code.

## Expected before

On gh-pages at 6a9171e, holds; after the change, fails:

- `curl -s https://beyond10x.github.io/takt/ | grep -c -i roadmap` prints 0.

## Scope

- `gh-pages` branch: `index.html` (cited)
- `docs/metrics/timeline.svg`, embedded (inferred; story:improvement-timeline-view)

## Acceptance

The live page has sections for the problem, how it works, status by milestone, measurements (each figure with n, date and source, and the line "first results, not a benchmark"), the improvement timeline from `docs/metrics/timeline.svg`, the roadmap and getting started; it names no person, customer or private repository, passes `b10x-gates scan-text`, and returns HTTP 200.
