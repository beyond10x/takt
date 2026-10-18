---
format: aep.planning-md/3
id: epic:recursive-self-improvement
kind: epic
status: draft
title: 'Recursive self-improvement: improvement turns as takt cases, judged by a promoted takt'
summary: Carries design:recursive-self-improvement as stories
relations:
- derived_from: design:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Epic: recursive-self-improvement

## Why

takt improves takt, the improvement process is itself a takt protocol, and the same loop
improves the loop. design:recursive-self-improvement is the design; this epic carries it as
stories.

## Acceptance

One promotion of a takt binary was decided by the scorecard, on validator evidence recorded in a
`self.improvement/1` case, and no scenario regressed against the previously promoted revision.

## Out of Scope

Phase R3 of the design (takt proposing stories from its own scorecards, the owner prioritising) is
deferred to a later epic: it starts after two weeks of R2 with no regression.

## Ordering against the hardening wave

Phase R2 of the design needs story:validator-confinement, story:tree-hygiene and
story:authority-hardening, and story:first-recorded-improvement-turn's candidate is
story:cli-protocol-defaults (all under epic:composable-agentic-work). On 2026-10-18 they are filed
on a branch that is not yet on main, so the `depends_on` edges from story:judge-guard,
story:self-improvement-protocol and story:first-recorded-improvement-turn to them are named in
those stories' Ordering sections; the edges are added with `aep plan artifact relate` once that
branch is on main.
