---
format: aep.planning-md/3
id: task:canon-0-8-0
kind: task
status: active
title: 'Canon moves to 0.8.0: sub-second order, and the whole-second wait goes'
summary: takt's Canon pin moves from 0.7.0 to 0.8.0, which orders records to the fraction of a second; takt drops the wait to the next whole second and writes tests_change and test_result at one precision.
tags:
- dependency
relations:
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T14:46:23Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T14:46:23Z", actor: "human:timo", revision: 3}
---
## What

Canon 0.8.0 orders two records by their instants to the fraction of a second, at one resolution
per evaluation, and adds the `since` order form. takt's Canon pin moves from 0.7.0 to 0.8.0, and
the wait that keeps two ordered records out of one whole second goes: `second_for`
(`crates/takt/src/evidence.rs`) and `now_after` with its bound `NOW_AFTER_BOUND`
(`crates/takt/src/lib.rs`).

takt then writes `tests_change` and `test_result` records at the same fractional precision. Where
the two kinds carry different precisions, Canon compares them in whole seconds, and two records in
one second are again of unknown order.

## Breaking for a consumer

- Canon's `Predicate` gains the variant `Since(EvidenceSince)`; an exhaustive match on it no
  longer compiles.

## Depends on

- loom's governor compiles protocols with Canon, and engineering-protocols builds on it: the move
  needs a loom release and an engineering-protocols release on Canon 0.8.0, or the workspace
  carries two Canon versions whose types do not unify. Check both before the wave opens.

## Not in scope

- The ESS pin stays where it is. The owner decided on 2026-10-29 not to move ESS with each ESS
  release, but once, when ESS's open issues are solved.

## Check

```expected_before
command: grep -q 'canon", tag = "0.8.0"' Cargo.toml && { echo "canon 0.8.0"; exit 0; } || { echo "canon older than 0.8.0"; exit 9; }
exit: 9
healthy_exit: 0
output: canon older than 0.8.0
source: Cargo.toml
```

## Acceptance

1. The predicate above exits 0.
2. `now_after` and `NOW_AFTER_BOUND` are gone, and `second_for` waits for no clock second.
3. A case that records `tests_change` and then `test_result` within one second admits the order
   its protocol requires, at the fractional precision both records carry; a test shows it.
4. The workspace builds and its tests pass.
