---
format: aep.planning-md/3
id: story:loop-completes-with-two-legitimate-outcomes
kind: story
status: draft
title: A case with two legitimate outcomes completes on the loop over loom-sdk
relations:
- decomposes: epic:takt-on-loom-sdk
revision: 1
---
# Story: loop-completes-with-two-legitimate-outcomes

## Outcome

A case whose protocol has two declared outcomes both legitimate at once ends `Completed` (exit 0)
on the loop over loom-sdk, as it did on takt's own loop at 7b54bad.

## Context

- From the grant review of wave 33 at a23e242 (review-result:wave-33-loop-a23e242), inferred from
  code, not observed: Loom's governor completes a case only when exactly one declared outcome is
  legitimate (loom `AGENTS.md` § Commission, `CompletionRule`), so the turn goes to
  `Taker::Nobody` (`crates/takt/src/run.rs:10299-10303`) and the run ends `NothingAdmissible`
  through `IdleBound` (`run.rs:10479-10511`).
- Loom offers `CompletionRule::FirstDeclared` to a host that builds the governor with it.

## Scope

- `crates/takt/src/run.rs`
- new fixture protocol and test under `crates/takt/tests/`

## Acceptance

```expected_before
command: a fixture protocol with two outcomes both met, `takt run` on it
exit: 3
healthy_exit: 0
source: review-result:wave-33-loop-a23e242
```

1. A test with a fixture protocol whose two outcomes are both met: the run stops `Completed`, exit 0.
2. First write the test and run it at a23e242: it shows whether the defect exists. If it passes there, the story closes as not reproduced.
