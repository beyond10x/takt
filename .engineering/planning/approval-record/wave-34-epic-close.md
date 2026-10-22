---
format: aep.planning-md/3
id: approval-record:wave-34-epic-close
kind: approval-record
status: approved
title: 'Wave 34 closes epic:takt-on-loom-sdk: Loom''s session executor, the evaluation code retired, the design'
relations:
- decides: story:takt-uses-loom-session-executor
- decides: story:takt-evaluation-code-retired
- decides: story:design-names-loom-as-the-runtime
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T16:10:51Z", actor: "human:timo", revision: 2}
---
# Approval: wave 34, the rest of epic:takt-on-loom-sdk

## Decision

The orchestrator, under the owner's delegation (2026-10-22), after acceptance 5 passed on 0.8.0:
"go to your next approved step on the ADR 0099 path (epic takt-on-loom-sdk) ... Approved under the
grant." The steps are those approval-record:takt-on-loom-sdk-closing-grant items 3 to 5 grant.

## The wave, as the dispatcher orders it

| unit | story | why in this order |
|---|---|---|
| U1 | story:takt-uses-loom-session-executor | first: `crates/takt/src/harness.rs` loses its own executor before U2 removes the `Harness` trait from the same file |
| U2 | story:takt-evaluation-code-retired | after U1, on the same files (`harness.rs`, `run.rs`, `ess/`, `AGENTS.md`); keeps what approval-record:loop-pre-checks-ahead-of-loom keeps |
| U3 | story:design-names-loom-as-the-runtime | last, by the doc-writer: it names the crates U1 and U2 leave |

The units run one after another in one tree, one build at a time; none runs beside another.

## Not in this wave

- story:loop-completes-with-two-legitimate-outcomes: no grant names it.
- story:harness-child-confined: stays blocked on its dependency.
- The release that closes the epic (closing grant item 5) follows the wave's merge.
