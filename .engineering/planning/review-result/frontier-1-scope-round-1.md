---
format: aep.planning-md/3
id: review-result:frontier-1-scope-round-1
kind: review-result
status: active
title: 'Scope critic, round 1: frontier-1 drafts'
tags:
- frontier-1
relations:
- reviews: story:baseline-runner-in-repo
- reviews: story:normalized-harness-trace
- reviews: story:wall-time-test-without-sleep
revision: 1
---
needs-revision
story:baseline-runner-in-repo — the summary promises the token shim, but neither Scope nor the acceptance names it; name it in both or drop it from the summary — .engineering/planning/story/baseline-runner-in-repo.md:7
story:normalized-harness-trace — the outcome repeats delivered work (story:run-trace-and-report's takt-trace/1 in one vocabulary, story:harness-journey-timeline's cross-harness lanes); what is missing is tool-call operations (ess/domains/takt.yaml:3398-3400 says none is recorded), a free-running harness's trace and a cross-run comparison of operation kinds — .engineering/planning/story/normalized-harness-trace.md:25
story:wall-time-test-without-sleep — the fixture half is delivered: crates/takt/tests/scorecard_adv1.rs:294 sets its instants with pin() since da068178; only the after-cleanup deadline test with an injected cleanup cost remains — .engineering/planning/story/wall-time-test-without-sleep.md:19

Recorded by the planner from the critic's round-1 return on the frontier-1 drafts. Each finding was checked against the tree at 338082d (branch wave/40) before the stories were revised; none was dropped.

```findings
[
{"file":".engineering/planning/story/baseline-runner-in-repo.md","line":7,"category":"scope","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the summary promises the token shim, but neither Scope nor the acceptance names it; name it in both or drop it from the summary"},
{"file":".engineering/planning/story/normalized-harness-trace.md","line":25,"category":"scope","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the outcome repeats delivered work (story:run-trace-and-report's takt-trace/1 in one vocabulary, story:harness-journey-timeline's cross-harness lanes); what is missing is tool-call operations (ess/domains/takt.yaml:3398-3400 says none is recorded), a free-running harness's trace and a cross-run comparison of operation kinds"},
{"file":".engineering/planning/story/wall-time-test-without-sleep.md","line":19,"category":"scope","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the fixture half is delivered: crates/takt/tests/scorecard_adv1.rs:294 sets its instants with pin() since da068178; only the after-cleanup deadline test with an injected cleanup cost remains"}
]
```
