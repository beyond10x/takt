---
format: aep.planning-md/3
id: story:the-gate-reads-paths-inside-arguments
kind: story
status: implemented
title: The gate finds the paths in glued short options and in shell command strings
summary: an edit to the file named by -fscripts/check.mk ran and recorded pass
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/gate_reads_paths_in_arguments.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T04:03:35Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T04:03:35Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-06T12:28:39Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Story: the-gate-reads-paths-inside-arguments

## Outcome

`gate.run` compares at the base the paths a gate command names inside an argument: a short option
with its value glued on (`-fscripts/check.mk`) and the words of a `sh -c "<line>"` string, or
refuses a gate command whose paths it cannot read.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): `argument_paths` takes only whole arguments, so an edit to `scripts/check.mk` named as
`-fscripts/check.mk` ran and recorded pass.

## Acceptance

- A step that edits the file in `-fscripts/check.mk` is refused before the gate runs: a test.
- A gate command `sh -c "<line>"` either has the paths in `<line>` compared or is refused at open,
  naming the command: a test.

## Status

Built in wave 55 (unit a, 24658b5a1, corrected in dbd7bba5f), released in 0.33.0 from fedfbc28c, the merge of pull request 62.

- `crates/takt/src/validator.rs`: `argument_paths` reads a single-dash argument with its value glued on and the path after `=` in a word; the words of a `-c` line of sh, bash and fish (`--command`) are compared at the base; a line that cannot be read with certainty, or sets `CDPATH`, is refused when the case opens.
- `crates/takt/tests/gate_reads_paths_in_arguments.rs` (red, then green) and the review cases in `w55_adversary.rs` and `w55_security.rs`.

Review: review-result:w55-adv-1, the `CDPATH`, fish and `key=path` findings fixed.
