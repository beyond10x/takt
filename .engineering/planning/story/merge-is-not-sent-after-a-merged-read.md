---
format: aep.planning-md/3
id: story:merge-is-not-sent-after-a-merged-read
kind: story
status: implemented
title: A merge is not sent after the read before it finds the pull request merged or closed
summary: the read just before the merge is checked only for its base
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T06:19:34Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T06:19:35Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-05T02:07:20Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# A merge is not sent after the read before it finds the pull request merged or closed

## What is wrong

Found by the adversary pass on wave 50's unit a (review-result:w50-adv-a-1, finding 3). Each
merge observation reads the pull request twice: the view, then a second read just before the
merge (`crates/takt/src/validator.rs:5894` at 28f467ab9). The second read is checked only for the
pull request's base. If it reports the pull request merged or closed, takt sends the merge
anyway. The adversary saw `view, view(merged), merge` at runtime, in the first run of its crash
case.

The fence of story:fenced-forge-dispatch stops a second send after a merge recorded `unknown` or
`applied`. It does not cover a first send that follows a read saying the pull request is no
longer open.

## Check

```expected_before
command: cargo test --locked -p b10x-takt --test fenced_forge_dispatch
exit: 0
healthy_exit: 0
output: test result: ok
source: review-result:w50-adv-a-1
```

No case reaches this yet. The story's first task is a case in which the second read reports the
pull request merged and the forge's call log shows no merge call.

## Outcome

When the read just before the merge finds the pull request merged, closed or at another head, no
merge is sent. The observation records what the read found, as the first read would have.

## Scope

- `crates/takt/src/validator.rs`: the read before the merge (cited)

## Status

Built in wave 53 (unit a, 73286bbd8), released in 0.31.0 from 410aaddd2, the merge of pull request 60.

- The read right before the merge in `crates/takt/src/validator.rs` (R7): merged or closed, it is observed as a first
  read finding it so, and no merge is sent; open at another head, nothing is merged and the claim stays unknown.
- `crates/takt/tests/merge_after_a_merged_read.rs`: merged, closed and moved-head second reads, each with no merge
  call in the forge's log (red, then green).

Review: review-result:w53-adv-2, no finding on this unit.
