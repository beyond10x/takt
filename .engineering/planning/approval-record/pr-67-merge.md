---
format: aep.planning-md/3
id: approval-record:pr-67-merge
kind: approval-record
status: approved
title: Merge of pull request 67 and the release 0.36.0
summary: the merge of wave 59 at its granted head and a release of it
relations:
- decides: story:a-connector-read-action
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-13T02:35:21Z", actor: "human:timo", revision: 2, executor: "agent:b10x-takt", correlation: "wave/59"}
---
# Approval: the merge of pull request 67 and the release 0.36.0

## Decision

The merge of pull request 67 (wave 59 of epic:scheduled-runs-and-connector-actions: story:a-connector-read-action,
with the close of wave 58) into `main` at head b47913d3, with its seven checks green, and a release 0.36.0 of it. It
touches judge surfaces the owner reserves (`ess/domains/takt.yaml` and approval records).

Granted by the owner on 2026-11-13 at 03:16 CET, by the owner's own answer (option A: "merge at head b47913d3 and tag
0.36.0") to a request that named that head. The merge is 32d02632, made at that head. The release needs a release
prepare (version, changelog, documentation) merged before the tag (pull request 68); the tag `0.36.0` goes on that merge.

An independent review of b47913d3 found no blocker and three should-fix findings, F1 to F3, and five minor ones, F4
to F8. The documentation of F1, F3, F4 and F7 is in the release prepare; F1 is the draft
story:connector-action-binds-an-action-not-declared-read; F4, F5 and F6 are acceptance lines of story:a-connector-write-under-a-grant; F8 is a citation added
in this close; F2 is the text added to approval-record:wave-58-command-action and approval-record:wave-59-connector-read-action.
