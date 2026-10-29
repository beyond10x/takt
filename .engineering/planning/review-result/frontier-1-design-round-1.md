---
format: aep.planning-md/3
id: review-result:frontier-1-design-round-1
kind: review-result
status: active
title: 'Design critic, round 1: frontier-1 drafts'
tags:
- frontier-1
relations:
- reviews: story:baseline-runner-in-repo
- reviews: story:promotion-records-its-toolchain
- reviews: story:scenario-corpus-realistic
revision: 1
---
needs-revision
story:baseline-runner-in-repo — the runner is committed as run-arm.sh and test-run-arm.sh; running code committed to a beyond10x repository is Rust with clap derive, so it is a Rust binary, a takt subcommand, or kept out of the repository — .engineering/planning/story/baseline-runner-in-repo.md:38
story:scenario-corpus-realistic — check-controls.sh and the per-scenario acceptance.sh are committed shell; running code is Rust with clap derive — .engineering/planning/story/scenario-corpus-realistic.md:42
story:baseline-runner-in-repo — docs/metrics/README.md is claimed by this story (the result.json mapping) and by story:scenario-corpus-realistic's frontmatter scope (scenario-corpus-realistic.md:13); only one may own it — .engineering/planning/story/baseline-runner-in-repo.md:26
story:promotion-records-its-toolchain — one story carries three outcomes: the toolchain identity in the record, crash-window recovery (stale .tmp-<pid>, orphan copy, record without install, lines 27 and 40-42) and the binary_sha256 digest check (lines 44-46); keep the toolchain and split the other two — .engineering/planning/story/promotion-records-its-toolchain.md:20

Recorded by the planner from the critic's round-1 return on the frontier-1 drafts. Each finding was checked against the tree at 338082d (branch wave/40) before the stories were revised; none was dropped.

```findings
[
{"file":".engineering/planning/story/baseline-runner-in-repo.md","line":38,"category":"design","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the runner is committed as run-arm.sh and test-run-arm.sh; running code committed to a beyond10x repository is Rust with clap derive, so it is a Rust binary, a takt subcommand, or kept out of the repository"},
{"file":".engineering/planning/story/scenario-corpus-realistic.md","line":42,"category":"design","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"check-controls.sh and the per-scenario acceptance.sh are committed shell; running code is Rust with clap derive"},
{"file":".engineering/planning/story/baseline-runner-in-repo.md","line":26,"category":"design","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"docs/metrics/README.md is claimed by this story (the result.json mapping) and by story:scenario-corpus-realistic's frontmatter scope (scenario-corpus-realistic.md:13); only one may own it"},
{"file":".engineering/planning/story/promotion-records-its-toolchain.md","line":20,"category":"design","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"one story carries three outcomes: the toolchain identity in the record, crash-window recovery (stale .tmp-<pid>, orphan copy, record without install, lines 27 and 40-42) and the binary_sha256 digest check (lines 44-46); keep the toolchain and split the other two"}
]
```
