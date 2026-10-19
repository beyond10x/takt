---
format: aep.planning-md/3
id: story:the-gate-reads-paths-inside-arguments
kind: story
status: draft
title: The gate finds the paths in glued short options and in shell command strings
summary: an edit to the file named by -fscripts/check.mk ran and recorded pass
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
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
