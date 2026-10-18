---
format: aep.planning-md/3
id: story:case-scoped-step-guard
kind: story
status: draft
title: A judgement step cannot undo another case's files
summary: The step guard covers the run's own case; a sibling's grant during the step is kept
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/git.rs
- confidence: inferred
  path: crates/takt/src/run.rs
revision: 3
---
# Story: case-scoped-step-guard

## Outcome

A judgement step on one case cannot undo another case's files: a `takt approve` on a sibling case during the step is kept, and the step is not refused for it.

## Context

Found by the second adversary pass over story:authority-hardening (`review-result:adversary-authority-pass-2`), pre-existing at ae94ccb: before a judgement step the run snapshots the whole cases directory and restores it afterwards (`crates/takt/src/run.rs:573-577` at 36f8405), while the run holds only its own case. With two cases in one tree (`takt run --case A` while B waits at its gate), an approve of B during A's step is reported as granted, then reverted; A's step is refused as if its harness had written B's `authority.yaml`, and A's work is discarded. Test: `adv2_a_grant_on_a_sibling_case_given_during_another_case_s_judgement_step_is_kept` in `crates/takt/tests/authority_adv2.rs`, which asserts today's behaviour until this story lands.

## Scope

- `crates/takt/src/run.rs`: the judgement-step section of `run_held` (inferred)
- `crates/takt/src/git.rs`: `Checkpoint` (inferred)

## Acceptance

With case B at its gate and a run on case A taking a judgement step, `takt approve B repository.merge` during the step leaves B's grant in place after the step, and A's step is judged only on what it changed; a write into A's own case directory is still refused and restored.
