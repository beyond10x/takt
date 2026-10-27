---
format: aep.planning-md/3
id: story:harness-none-takes-a-granted-action
kind: story
status: implemented
title: --harness none finishes a case after a grant
summary: NoHarness takes an admissible granted action instead of ending on an error
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T02:32:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-27T02:32:06Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-27T07:37:24Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: harness-none-takes-a-granted-action

## Outcome

`--harness none` can finish a case after a grant: resumed with a granted action admissible, it
takes that action instead of ending the run on an error.

## Context

- Observed 2026-10-26 on takt 0.14.0, written into `website/docs/guides/approve-a-capability.md:61-67`:
  `takt run --case <c> --harness none --forge none` after `takt approve` ends `stopped: error:
  step 1: the harness failed ... no harness: a judgement step among implementation.edit,
  repository.merge, tests.write is left`.
- `NoHarness` (`crates/takt-cli/src/main.rs:902-935`) requests the gate action only while an
  approval is pending (`context.approvals`), and takes a choice action; after the grant the merge
  is admissible, not approval-required, so neither branch matches and it returns the error.
- With `--forge none` the merge itself is `story:forge-none-merges-locally`.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check`.

1. `harness-none/takes-the-granted-merge`: a case granted `repository.merge` at its revision,
   resumed with `--harness none` and a forge double that merges, takes `repository.merge` and
   stops `Completed`, exit 0.
2. `harness-none/no-grant-unchanged`: with no grant, `--harness none` behaves as today: it requests
   a pending approval, takes a choice, and otherwise ends `stopped: error:` naming the steps left.

## Scope

Inferred, not confirmed: `crates/takt-cli/src/main.rs` (`NoHarness`), `takt::StepContext` in
`crates/takt/src/harness.rs`, `crates/takt-cli/tests/`.
