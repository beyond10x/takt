---
format: aep.planning-md/3
id: approval-record:acceptance-5-baseline-and-corpus
kind: approval-record
status: approved
title: Acceptance 5 on 0.8.0 is measured against promotion 7ae5b05b over scenario 1
relations:
- decides: story:the-loop-is-run-until-blocked
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T16:10:03Z", actor: "human:timo", revision: 2}
---
# Approval: the baseline and corpus of acceptance 5 on takt 0.8.0

## Decision

The trial agent's two decisions for acceptance 5 of story:the-loop-is-run-until-blocked, filed by the
dispatcher at the orchestrator's request under the owner's delegation (2026-10-22).

| id | decision | why |
|---|---|---|
| D1 | the baseline is promotion 7ae5b05b, rebaseline-94a60cb9 | the last promotion before takt pinned `b10x-loom-sdk` (0fab445d is the first revision to pin it) |
| D2 | the corpus is scenario 1 only | every promoted scorecard holds scenario 1 alone |

## Result

`takt scorecard --against` at takt 0.8.0 (657516dd) exited 0, not worse:

| figure | value |
|---|---|
| trials | 5 on scenario 1; each stopped `ApprovalRequired`, exit 4 |
| median tokens, candidate | 48772 |
| median tokens, saved at 7ae5b05b | 66421 |
| against f4f8d8d2 | median tokens -165, not worse, exit 0 |

The case with two legitimate outcomes (story:loop-completes-with-two-legitimate-outcomes) was not
exercised: scenario 1 has one outcome.
