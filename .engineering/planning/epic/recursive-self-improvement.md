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
revision: 3
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

story:validator-confinement, story:tree-hygiene, story:authority-hardening and the first turn's
candidate story:cli-protocol-defaults are implemented on main (epic:composable-agentic-work), and
the `depends_on` edges from story:judge-guard and story:self-improvement-protocol to them are
recorded.

The path to this epic's acceptance, as of 2026-10-19:

1. story:promotion: `takt promote` installs a binary only on a no-regression scorecard.
2. story:judge-guard: the promoted takt judges takt's cases; a judge-surface merge needs the
   owner's grant or a delegation on record.
3. story:self-improvement-protocol: validators write every record of a `self.improvement/1` case.
   The epic's acceptance is met by the first case this story drives to `accepted` whose
   `binary.promote` is decided by the scorecard.

Beside that path: story:loop-scorecard, then story:loop-case-opener (the L2 turn through `takt
run`); story:improvement-turns-2-and-3-recorded, once the first promotion after wave 12 is
installed, gives story:loop-scorecard its three recorded turns.
