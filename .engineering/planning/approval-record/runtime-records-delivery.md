---
format: aep.planning-md/3
id: approval-record:runtime-records-delivery
kind: approval-record
status: accepted
title: Runtime-records units merge at green and are reviewed on integration/takt
tags:
- wave-approval
relations:
- decides: story:cases-live-under-dot-takt
- decides: story:the-state-directory-holds-every-host-record
- decides: story:a-run-keeps-every-decision-only-on-a-trial
- decides: story:python-modules-are-kept-only-when-a-python-answers
- decides: story:the-pull-request-and-the-store-carry-the-evidence
revision: 2
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T02:33:42Z", actor: "human:timo", revision: 2}
---
Decider: operator

Changes how the five runtime-records stories are delivered onto integration/takt. Their scope stays as accepted in
approval-record:runtime-records-accept.

Quoted operator words, 2026-10-27, relayed by the orchestrator: "make delivery faster".

The orchestrator's ruling on them, 2026-10-27, verbatim: "unit/rr-python merges into integration/takt and is pushed
now that it is green (fetch, merge origin/integration/takt, re-run affected suites, fast-forward push, never force);
its adversary keeps running against integration/takt and findings land there as fixes before any PR. Start the other
stories in parallel as soon as their depends_on are merged; units needing --state-dir start once the carry is on
integration/takt. My 01:46Z \"one cargo tree at a time\" order is withdrawn: builds run in parallel inside 40G for
takt; disk-guard covers the floor. ... Still no PR, no merge to main."

What changes:

- A unit merges into integration/takt and is pushed once its tests and clippy are green. Its adversary and
  security-review passes then run against integration/takt, and their findings land there as fix commits or stories
  before any pull request.
- A unit starts as soon as the stories it depends on are merged. Units that use the configurable state directory
  start once that directory is on integration/takt.
- Builds of different units may run at the same time.
- Unchanged: no pull request, no merge to the base branch, no release.
