---
format: aep.planning-md/3
id: story:pr-open-cut-off-by-a-crash-is-read-back
kind: story
status: draft
title: A pull request open cut off by a crash is read back on every forge
summary: only the connectors forge finds the open pull request after a crash
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A pull request open cut off by a crash is read back on every forge

## What is wrong

Found while correcting wave 50's unit a (story:fenced-forge-dispatch). `pr.open` records its
write as `unknown` before the request leaves. After a crash, the next run resolves it by asking
the forge for an open pull request from the case's branch (`Forge::find_open_pull_request`). Of
the shipped forges, only `connectors` answers that question; it keys each open so a lost answer
replays. The `command`, GitLab and local forges keep the trait's default and find nothing. A
pull request the crashed run opened is then opened a second time.

## Check

```expected_before
command: cargo test --locked -p b10x-takt --test fenced_forge_dispatch
exit: 0
healthy_exit: 0
output: test result: ok
source: review-result:w50-adv-a-1
```

No case reaches this yet. The story's first task is a case for each of those forges: a `pr.open`
whose process ends after the post, then a resumed run, with the forge's call log showing one open.

## Outcome

Every shipped forge finds the open pull request for a branch, or refuses to open one while an
`unknown` open stands for that branch, so a crash never leaves two pull requests.

## Scope

- `crates/takt/src/forge/command.rs`, `forge/gitlab.rs`, `forge/local.rs`: `find_open_pull_request` (inferred)
