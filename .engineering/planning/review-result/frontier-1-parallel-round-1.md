---
format: aep.planning-md/3
id: review-result:frontier-1-parallel-round-1
kind: review-result
status: active
title: 'Parallel-safety critic, round 1: frontier-1 drafts'
tags:
- frontier-1
relations:
- reviews: story:abandon-a-case
- reviews: story:normalized-harness-trace
- reviews: story:promotion-records-its-toolchain
- reviews: story:scenario-corpus-realistic
- reviews: story:wall-time-test-without-sleep
revision: 1
---
needs-revision
story:abandon-a-case — the story lands on crates/takt/src/run.rs and crates/takt/src/validator.rs, which wave 39 edits, and says nothing about ordering; it waits for wave 39 to merge — .engineering/planning/story/abandon-a-case.md:45
story:normalized-harness-trace — crates/takt/src/harness.rs is wave 39's file and the story does not say so; crates/takt/src/trace.rs, the module the trace lives in, is missing from Scope — .engineering/planning/story/normalized-harness-trace.md:18
story:promotion-records-its-toolchain — the explicit toolchain-change flag needs PromoteArgs in crates/takt-cli/src/main.rs and the record's fields need ess/domains/takt.yaml; neither is in Scope — .engineering/planning/story/promotion-records-its-toolchain.md:31
story:scenario-corpus-realistic — the frontmatter scope names docs/metrics/README.md while the body's Scope names docs/metrics/scenarios/*, so the computed waves assess the wrong surface — .engineering/planning/story/scenario-corpus-realistic.md:13
story:wall-time-test-without-sleep — the after-cleanup test lands on crates/takt/src/run.rs (wave 39's file) and crates/takt/tests/hours_bounds_a_harness_step_adv1.rs, neither in Scope; the note that story:run-trace-and-report also edits scorecard_adv1.rs is stale, that story is implemented — .engineering/planning/story/wall-time-test-without-sleep.md:27

Recorded by the planner from the critic's round-1 return on the frontier-1 drafts. Each finding was checked against the tree at 338082d (branch wave/40) before the stories were revised; none was dropped.

```findings
[
{"file":".engineering/planning/story/abandon-a-case.md","line":45,"category":"parallel-safety","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the story lands on crates/takt/src/run.rs and crates/takt/src/validator.rs, which wave 39 edits, and says nothing about ordering; it waits for wave 39 to merge"},
{"file":".engineering/planning/story/normalized-harness-trace.md","line":18,"category":"parallel-safety","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"crates/takt/src/harness.rs is wave 39's file and the story does not say so; crates/takt/src/trace.rs, the module the trace lives in, is missing from Scope"},
{"file":".engineering/planning/story/promotion-records-its-toolchain.md","line":31,"category":"parallel-safety","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the explicit toolchain-change flag needs PromoteArgs in crates/takt-cli/src/main.rs and the record's fields need ess/domains/takt.yaml; neither is in Scope"},
{"file":".engineering/planning/story/scenario-corpus-realistic.md","line":13,"category":"parallel-safety","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the frontmatter scope names docs/metrics/README.md while the body's Scope names docs/metrics/scenarios/*, so the computed waves assess the wrong surface"},
{"file":".engineering/planning/story/wall-time-test-without-sleep.md","line":27,"category":"parallel-safety","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the after-cleanup test lands on crates/takt/src/run.rs (wave 39's file) and crates/takt/tests/hours_bounds_a_harness_step_adv1.rs, neither in Scope; the note that story:run-trace-and-report also edits scorecard_adv1.rs is stale, that story is implemented"}
]
```
