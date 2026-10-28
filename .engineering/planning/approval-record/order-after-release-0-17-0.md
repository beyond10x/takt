---
format: aep.planning-md/3
id: approval-record:order-after-release-0-17-0
kind: approval-record
status: approved
title: The order of takt's work after release 0.17.0, pull request 17 and the installed takt
summary: Trial baseline, real-story deliveries, release and trial, plan critics, next wave; PR 17 closed; takt replaced after the trial
relations:
- decides: epic:run-config-proxies-llm-and-metaharness
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T15:18:29Z", actor: "human:timo", revision: 3}
---
# Approval: the order of takt's work after release 0.17.0

## Decision

The orchestrator, under the owner's grant (2026-10-28: decisions are taken on the owner's behalf),
after a reconcile of the repository's branches, trees and store at `main` c1a6d3c.

| item | decision |
|---|---|
| order 1 | a trial cycle on takt 0.17.0 runs to its end; its scorecards are the baseline for the next release |
| order 2 | the fixes of the first real-story delivery land on `integration/takt`, with engineering-protocols 0.12.1, then two real-story deliveries on the default protocol with no workaround; if they are handed to this repository's sessions, they rank before any trial |
| order 3 | a takt release from `integration/takt` through this repository's release process, then a trial of it against the 0.17.0 baseline |
| order 4 | the plan critics read epic:run-config-proxies-llm-and-metaharness; the stories that pass are accepted |
| order 5 | the next wave takes `rsi` trial findings first, then the draft frontier's first wave; at most four units, one shared target directory, and it starts only with 25 GB or more free on the build host |
| order 6 | story:harness-child-confined waits for a Loom release that closes the gaps G1 to G5 of dependency-blocker:loom-child-confinement |
| pull request 17 | closed through the bot route (`state: closed`); its branch `autodev/s5-2026-10-26` stays as the record of that trial. It was produced by takt 0.12.0, its citations gate fails and its body says not to merge it |
| installed takt | once the 0.17.0 cycle has ended and no process runs takt, the newest takt release replaces the installed 0.7.0; not while a trial runs |

The deletion of local trees, a stash and unpushed branches is not decided here: it deletes data,
which the grant does not cover (decision-blocker:local-leftovers-after-wave-38).
