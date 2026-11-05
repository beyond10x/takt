---
format: aep.planning-md/3
id: story:gate-flag-names-a-protocol-action
kind: story
status: active
title: --gate must name an approval-required action of the protocol
tags:
- finding
refs:
- provider: finding
  reference: gate-flag-names-a-protocol-action
relations:
- informed_by: review-result:findings-hunt-33ab42c
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:14:03Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T02:14:04Z", actor: "human:timo", revision: 3}
---
## Outcome

A `--gate` that names no approval-required action of the protocol the case runs on is refused with exit 1 before the case opens.

## Context

`--gate` is copied into `RunOptions.gate` (`main.rs:524`) and only used as a filter (`run.rs:3759`), so a typo means the gate is never offered. Observed with `--gate no.such.action`:
- the case opened on `software.change/1`;
- `repository.merge` became approval-required but was never offered;
- the run ended `stopped: error: … no harness`, exit 5.

A second flag is ignored the same way: `--case-id zzz` given with `--case a` resumed `a` and created no `zzz`, while `--protocol` and `--input` are refused on a resume (`main.rs:1217-1245`).

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent probe --harness none --forge none --iterations 3 --test-cmd ./check.sh --gate no.such.action
exit: 5
output: on software.change/1
source: .engineering/planning/review-result/findings-hunt-33ab42c.md
```

## Scope

`crates/takt-cli/src/main.rs` (`LoopArgs::check`, `RunArgs` handling; refusing `--case-id` together with `--case`).

## Acceptance

The predicate exits 1 and names the protocol's approval-required actions, and no case directory exists. `--case a --case-id zzz` exits 1.
