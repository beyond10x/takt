---
format: aep.planning-md/3
id: story:forge-none-help-names-the-unperformable-stop
kind: story
status: draft
title: takt run --help says a --forge none run stops Unperformable, exit 7, instead of leaving the forge actions to the harness
summary: the --forge none help says pr.open, ci.read and repository.merge are left to the harness; the run stops exit 7
tags:
- finding
- rsi
refs:
- provider: finding
  reference: forge-none-help-names-the-unperformable-stop
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: forge-none-help-names-the-unperformable-stop

## Outcome

`takt run --help` says what a `--forge none` run does when `pr.open`, `ci.read` or
`repository.merge` is the only way on: it stops `Unperformable`, exit 7, and the case stays open.
It no longer says those actions "are left to the harness".

## Context

Found by the wave 15 close review at 0fae6305 (docs/waves/wave-15.md). The `--forge` doc comment
(`crates/takt-cli/src/main.rs:407` at 0fae6305, unchanged on main) still reads "`none` (no pull
request is opened; `pr.open`, `ci.read` and `repository.merge` are left to the harness)". Since
wave 15 the run does not leave them to the harness: with no performer for the deciding action it
stops `Unperformable`, exit 7 (`main.rs:1312`; exit code 7 in the exit codes text of `takt run --help`).
The review's five scenario-1 trials of the tip with no forge each stopped `Unperformable`, exit 7.
An operator who reads the help expects a harness step for the merge and gets a stop instead.

## Acceptance

1. The `--forge` help names, for `none`, the `Unperformable` stop and exit 7 when the deciding
   action needs the forge; the words "left to the harness" are gone: a test on the help text.
2. At the candidate, `scripts/check-finding story:forge-none-help-names-the-unperformable-stop
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: "$TAKT" run --help
exit: 0
healthy_exit: 0
output: are left to the harness
source: docs/waves/wave-15.md
```
