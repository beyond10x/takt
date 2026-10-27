---
format: aep.planning-md/3
id: approval-record:release-0-16-0
kind: approval-record
status: approved
title: 'Release 0.16.0 from integration/takt at e98a4980: confinement through Substrate'
summary: The live trials on 0.15.1 are done; 0.16.0 releases the wave and the Substrate hop
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-27T19:55:16Z", actor: "human:timo", revision: 3}
---
# Approval: release 0.16.0

## Decision

The orchestrator, under the owner's grant, 2026-10-27 after the three live trials on 0.15.1 ended
with their pull requests merged: release 0.16.0 from `integration/takt` at e98a4980. The hold on
merging to `main` while the trials ran is over.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.16.0 | three `**Breaking:**` lines: the defaults document, confinement through Substrate, `--confinement none` on Substrate's `local` adapter |
| revision | `integration/takt` at e98a4980 plus one release commit, merged to `main` through a pull request | the wave's targeted tests, a full suite of 2257 passed and 0 failed, and darwin green on that head |
| ESS | the checks stay on `ess` 0.70.0 | adopting a newer ESS is a separate wave |
| install lines | the tag's commit is written as a placeholder until the merge names it | the release commit cannot name the merge it goes into |

## Steps

1. A pull request from `integration/takt` to `main` through the bot; every required check green,
   darwin included.
2. Merge, tag 0.16.0 on the merge, the release workflow, then its verification.
