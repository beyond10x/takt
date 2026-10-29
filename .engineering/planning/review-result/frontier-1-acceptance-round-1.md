---
format: aep.planning-md/3
id: review-result:frontier-1-acceptance-round-1
kind: review-result
status: active
title: 'Acceptance critic, round 1: frontier-1 drafts'
tags:
- frontier-1
relations:
- reviews: story:abandon-a-case
- reviews: story:baseline-runner-in-repo
- reviews: story:normalized-harness-trace
- reviews: story:promotion-records-its-toolchain
revision: 1
---
needs-revision
story:abandon-a-case — the acceptance joins two outcomes, the takt abandon verb and a closed pull request stopping the run; each needs its own story and acceptance — .engineering/planning/story/abandon-a-case.md:53
story:abandon-a-case — the expected-before probe grep -rn explicitly_abandoned crates/ already prints test fixtures (crates/takt/tests/pr_opens_when_done.rs:810), so it does not hold before the change; grep crates/takt/src crates/takt-cli/src instead — .engineering/planning/story/abandon-a-case.md:42
story:abandon-a-case — protocols/story-delivery/1.yaml:155-158 is not in the tree; story.delivery/1 is embedded from the b10x-canon-engineering crate (engineering-protocols 0.12.0, protocols/story-delivery/1.yaml:210-213) — .engineering/planning/story/abandon-a-case.md:36
story:abandon-a-case — run.rs:477-491, run.rs:516-522 and validator.rs:756-781 are stale; the resume is now run.rs:9720-9784, the skip run.rs:9852-9858, pr.open's state read validator.rs:3359-3386 — .engineering/planning/story/abandon-a-case.md:34
story:baseline-runner-in-repo — the acceptance puts a codex double on PATH as scenario_run.rs builds one, but crates/takt-cli/tests/scenario_run.rs builds only b10x-gates, gh and claude doubles; the codex double is new work the story does not name — .engineering/planning/story/baseline-runner-in-repo.md:44
story:normalized-harness-trace — the expected-before (no trace file, one summed token figure) is stale: takt trace writes a takt-trace/1 per case with token components from raw usage (crates/takt/src/trace.rs:1-16), so the predicate does not hold before the change — .engineering/planning/story/normalized-harness-trace.md:41
story:promotion-records-its-toolchain — no acceptance statement observes the record's new toolchain fields; only the refusals are checked — .engineering/planning/story/promotion-records-its-toolchain.md:38

Recorded by the planner from the critic's round-1 return on the frontier-1 drafts. Each finding was checked against the tree at 338082d (branch wave/40) before the stories were revised; none was dropped.

```findings
[
{"file":".engineering/planning/story/abandon-a-case.md","line":53,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the acceptance joins two outcomes, the takt abandon verb and a closed pull request stopping the run; each needs its own story and acceptance"},
{"file":".engineering/planning/story/abandon-a-case.md","line":42,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the expected-before probe grep -rn explicitly_abandoned crates/ already prints test fixtures (crates/takt/tests/pr_opens_when_done.rs:810), so it does not hold before the change; grep crates/takt/src crates/takt-cli/src instead"},
{"file":".engineering/planning/story/abandon-a-case.md","line":36,"category":"citation","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"protocols/story-delivery/1.yaml:155-158 is not in the tree; story.delivery/1 is embedded from the b10x-canon-engineering crate (engineering-protocols 0.12.0, protocols/story-delivery/1.yaml:210-213)"},
{"file":".engineering/planning/story/abandon-a-case.md","line":34,"category":"citation","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"run.rs:477-491, run.rs:516-522 and validator.rs:756-781 are stale; the resume is now run.rs:9720-9784, the skip run.rs:9852-9858, pr.open's state read validator.rs:3359-3386"},
{"file":".engineering/planning/story/baseline-runner-in-repo.md","line":44,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the acceptance puts a codex double on PATH as scenario_run.rs builds one, but crates/takt-cli/tests/scenario_run.rs builds only b10x-gates, gh and claude doubles; the codex double is new work the story does not name"},
{"file":".engineering/planning/story/normalized-harness-trace.md","line":41,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the expected-before (no trace file, one summed token figure) is stale: takt trace writes a takt-trace/1 per case with token components from raw usage (crates/takt/src/trace.rs:1-16), so the predicate does not hold before the change"},
{"file":".engineering/planning/story/promotion-records-its-toolchain.md","line":38,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"no acceptance statement observes the record's new toolchain fields; only the refusals are checked"}
]
```
