---
format: aep.planning-md/3
id: story:pr-open-cut-off-by-a-crash-is-read-back
kind: story
status: implemented
title: A pull request open cut off by a crash is read back on every forge
summary: only the connectors forge finds the open pull request after a crash
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/forge/command.rs
- confidence: inferred
  path: crates/takt/src/forge/gitlab.rs
- confidence: inferred
  path: crates/takt/src/forge/local.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T06:19:35Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T06:19:35Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-05T02:07:20Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
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

## Decision

Decided 2026-11-04 under the owner's standing delegation, through the orchestrator: a command forge
without a `find` entry refuses every later open in a case once an open's answer stayed `unknown`.
The record `open.unanswered.json` under the case's artifacts names that open; the refusal names
`find`. It lifts when the run file gains `find`, or when the operator removes the record once the
forge holds no open request for the branch. Refusing is the safe direction: sending the open again
could leave two pull requests. The 0.31.0 changelog states it under Changed, with both ways out.

## Status

Built in wave 53 (unit b, 302cd81d2; review fix c171df987; fixture fixes 9e760b2fe, bfb570639), released in 0.31.0
from 410aaddd2, the merge of pull request 60.

- The local and GitLab forges implement `find_open_pull_request`; the command forge has an optional `find` entry,
  and its number, like `open`'s, is taken only when `read` reports it open from the case's branch into its base.
- Without `find`, `open.unanswered.json` names an open whose answer stayed unknown, and every later open in the
  case is refused, naming `find` (the Decision above).
- The local forge writes each record whole to `<n>.json.<pid>.next` and links it to `<n>.json`.
- `pr_open_after_a_crash_on_every_forge.rs` (red, then green); new cases in `command_forge.rs` and `local_forge.rs`.

Review: review-result:w53-adv-2, fixed.
