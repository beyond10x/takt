---
format: aep.planning-md/3
id: story:repeated-identical-test-failures-stop-the-run
kind: story
status: implemented
title: Identical test failures across revisions stop the run
summary: 18 tests.run failed on the same four tests while the model edited on, 13.8M tokens (second kind-cluster trial)
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T20:53:18Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T20:53:53Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-26T22:53:17Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A delivery whose test command fails on the same tests at every revision for a stated number of
edits stops with a stop that names those tests, instead of editing on until a budget ends.

## Context

- The second kind-cluster trial, 2026-10-26 (takt 0.13.1 on its own repository, story:git-excluded-files-are-not-step-changes):
  runs 7 and 8 took 42 and 44 steps, 18 `tests.run` in all, every one `126 passed, 4 failed` on
  the same four `harness::endpoint_tests` tests, which already failed at the case's first revision d6516a7a (the base e5499f76 and the take) in that
  environment; 13,792,661 tokens across the two runs. The model edited and handed over 15 times
  (`change.handover`, `change.read`: ahead) without touching those tests.
- story:a-red-base-stops-at-open covers a base that is red before the first model call; this
  story covers the loop after it, when the failing set never changes.

## Acceptance

1. After N consecutive `tests.run` results that fail on the same set of tests at different
   revisions (N stated), the run stops exit 3 naming the tests and N; the case stays open.
2. A run whose failing set changes between results does not stop for this.
