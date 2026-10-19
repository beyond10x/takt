---
format: aep.planning-md/3
id: story:disk-low-exits-with-its-own-code
kind: story
status: draft
title: A run that stops DiskLow exits 8, not the 3 that NoProgress and the budget stops give
summary: DiskLow exits 3 like NoProgress, so runners resume a trial they should discard
tags:
- finding
- rsi
refs:
- provider: finding
  reference: disk-low-exits-with-its-own-code
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: disk-low-exits-with-its-own-code

## Outcome

A run that stops `DiskLow` exits 8, a code of its own, so a runner can tell a trial the machine
spoiled from one takt stopped. Exit 3 keeps the budget stops, `NothingAdmissible`, `StepRefused`
and `NoProgress`.

## Context

Found by the wave 15 close review at 0fae6305 (docs/waves/wave-15.md).
`crates/takt-cli/src/main.rs:1313` at 0fae6305 maps `Stop::DiskLow` to `EXIT_STOPPED` (3), and the
exit codes text of `takt run --help` lists `DiskLow` under 3. The trial runners resume a case
that exits 3, as a budget stop asks for. A `DiskLow` case is the wrong one to
resume: the scorecard treats any case with a `DiskLow` run as a `DiskLow` trial, whatever its
later runs did (`DISK_LOW`, `crates/takt/src/scorecard.rs:68`), and the judge gives the scenario
no verdict until a fresh set without one is saved. So the resume spends a trial's time and still
yields a trial that has to be thrown away. A distinct exit code lets the runner discard it at once.

The stop is deterministic: `--min-free-gib 100000` stops before the first step on any machine
(review probe: `stopped: DiskLow (...: 43.09 GiB free, under 100000.00 GiB); steps 0`).

## Acceptance

1. `takt run` and `takt scenario run` exit 8 on a `DiskLow` stop; every other stop keeps its code:
   a test for each command.
2. The exit codes text of `takt run --help` and the `--min-free-gib` help name 8 for `DiskLow`, and
   3 no longer lists it.
3. At the candidate, `scripts/check-finding story:disk-low-exits-with-its-own-code <candidate>`
   prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent "Remove a todo by its number" --harness none --forge none --test-cmd ./check.sh --min-free-gib 100000
exit: 3
healthy_exit: 8
output: stopped: DiskLow (
source: docs/waves/wave-15.md
```
