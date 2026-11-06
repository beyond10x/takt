---
format: aep.planning-md/3
id: story:the-step-guard-reads-key-path-words-as-the-gate-does
kind: story
status: draft
title: The step guard reads a path after = in a test command's line, as the gate does
summary: BASH_ENV=scripts/env and dd if= are compared by the gate since wave 55, not by the step guard
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: the-step-guard-reads-key-path-words-as-the-gate-does

## Outcome

The step guard's reader of a test command's line (`named_from` in `crates/takt/src/run.rs`) compares the path after
`=` in a word that is no option (`dd if=rules.txt`, `BASH_ENV=scripts/env`), as the gate's reader does since wave 55.

## Context

Found by the wave 55 security review at d0e8fba54 and left by its correction (dbd7bba5f), which changed the gate's
reader in `crates/takt/src/validator.rs` only. `BASH_ENV` runs code from the tree.

## Acceptance

- A test command `sh -c 'BASH_ENV=scripts/env sh check.sh'` whose `scripts/env` a step edited is set aside or refused
  as a change to the test command's surface: a test.
