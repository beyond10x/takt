---
format: aep.planning-md/3
id: story:the-gate-compares-taskfile-includes
kind: story
status: implemented
title: The gate compares the files a Taskfile includes at the base
summary: 'an edit to a file a Taskfile pulls in through includes: ran and recorded pass'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/gate_compares_taskfile_includes.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T04:03:35Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T04:03:35Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-06T12:28:41Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Story: the-gate-compares-taskfile-includes

## Outcome

`gate.run` compares at the base every file a compared Taskfile pulls in through `includes:`, so a
step cannot change a gate command by editing an included Taskfile.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): an edit to `tasks/ci.yml`, included from `Taskfile.yml`, ran and recorded pass.

## Acceptance

- A step that edits a Taskfile named in `includes:` is refused before the gate runs, naming the
  file: a test.

## Status

Built in wave 55 (unit b, 1f828f8e6), released in 0.33.0 from fedfbc28c, the merge of pull request 62.

- `crates/takt/src/validator.rs`: the gate reads `includes:` of every Taskfile at the base, resolves each against the including file and compares every included file at the base; an include it cannot resolve, and a base Taskfile with an anchor or alias, refuse the gate (approval-record:wave-55-taskfile-anchors).
- `crates/takt/tests/gate_compares_taskfile_includes.rs` (red, then green).

Review: review-result:w55-adv-1; a Taskfile named by `task -t` is story:the-gate-follows-the-includes-of-a-taskfile-named-by-argument.
