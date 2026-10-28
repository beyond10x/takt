---
format: aep.planning-md/3
id: approval-record:wave-trial-0-16-findings
kind: approval-record
status: approved
title: The 0.16.0 trial findings wave and release 0.17.0
relations:
- decides: story:a-run-can-decline
- decides: story:a-scenario-declares-an-expected-red-base
- decides: story:an-accepted-run-reports-the-tested-revisions-obligations
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T06:52:34Z", actor: "human:timo", revision: 3}
---
# Approval: the 0.16.0 trial findings wave and release 0.17.0

## Decision

The orchestrator, under the owner's grant (2026-10-28): "a takt wave on integration/takt ... for 4
findings from the live trials of todo-app-example scenarios 2-6 on takt 0.16.0 ... draft stories
in the takt store (accept them under this grant as the planner role allows), AEP wave mode ...
Then release takt 0.17.0 from integration/takt to main through takt's own process ... Also move
takt's ESS pin to the newest released ESS (0.73.0 now; 0.74.0 if tagged by then) in this wave."

## Covered

| item | what |
|---|---|
| finding 1 | story:a-run-can-decline: a refused story stops `Declined`, exit 9, apart from `NoProgress` |
| finding 2 | story:a-scenario-declares-an-expected-red-base: `red_base = expected` in `scenarios.conf` |
| finding 3 | story:an-accepted-run-reports-the-tested-revisions-obligations |
| finding 4 | no story: `takt scenario run` opens its trial case on `story.delivery/1` by design (`SCENARIO_PROTOCOL`, a trial case sets no input), while `takt run` on a store story opens on `story.delivery/2`; the 0.17.0 CHANGELOG corrects the 0.16.0 line |
| pin | the ESS pin moves to 0.73.0, a pin-only unit with no adversary pass |
| release | takt 0.17.0 from `integration/takt` to `main` through a pull request, merged through the App when its required checks are green |

One plan-critic round (acceptance, design, parallel safety); its findings were folded into the
drafts before they were accepted.
