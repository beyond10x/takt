---
format: aep.planning-md/3
id: story:scenario-run-help-exit-codes
kind: story
status: draft
title: takt scenario run --help lists only the exit codes that verb gives
tags:
- finding
refs:
- provider: finding
  reference: scenario-run-help-exit-codes
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
revision: 2
---
# Story: scenario-run-help-exit-codes

## Outcome

`takt scenario run --help` lists the exit codes that verb gives, and none it never gives: it no longer says exit 0 can mean "frontier: nothing ready", which only `takt run` prints.

## Context

Split out of story:cli-small-fixes, item (b), found by an independent code review of 2026-10-18: `takt scenario run` shares `takt run`'s exit-code help text (`crates/takt-cli/src/main.rs`, the shared `after_help`), so its `--help` line for 0 reads `stopped: Completed (an outcome is legitimate), or frontier: nothing ready`. A scenario run never consults the frontier for the next ready story: it refuses a scenario whose story is not ready before anything is pushed, with exit 1.

This is the finding of the first recorded improvement turn (design:recursive-self-improvement, phase R0): it is checkable offline by `scripts/check-finding`, so its before and after are machine results.

```expected_before
scenario: 1
command: "$TAKT" scenario run --help
exit: 0
output: frontier: nothing ready
source: .engineering/planning/story/cli-small-fixes.md
```

## Scope

- `crates/takt-cli/src/main.rs`: the `scenario run` command's `after_help` (inferred)

## Acceptance

`takt scenario run --help` exits 0 and its exit-code list holds no line with `frontier: nothing ready`; `takt run --help` still lists it; `scripts/check-finding` on this finding prints `reproduced` at 0a776d2 and `not_reproduced` at the fix.
