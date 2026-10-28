---
format: aep.planning-md/3
id: story:an-unrelated-program-install-refuses-no-step
kind: story
status: active
title: A program installed by another process during a step refuses that step
tags:
- finding
- priority:medium
refs:
- provider: finding
  reference: an-unrelated-program-install-refuses-no-step
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/tests/approval_riders_adv2.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T21:15:41Z", actor: "human:timo", revision: 4, executor: "agent:planner"}
- {from: "proposed", to: "active", at: "2026-10-28T21:15:41Z", actor: "human:timo", revision: 5, executor: "agent:developer"}
---
# A program installed by another process during a step refuses that step

## What is wrong

In wave 39's gate, `approval_riders_adv2` (`crates/takt/tests/approval_riders_adv2.rs:233` at
7c9e7129) stopped `StepRefused("ci.read wrote what its validator does not:
<home>/.cargo/bin/ess, not put back (a program or import directory takt never writes, …)")`
instead of `ApprovalRequired(["repository.merge"])`. The `ess` binary was written by an
unrelated install that ran on the same machine during the test, not by the step; the test passed
alone at the same revision (`docs/waves/wave-39.md`).

Two things follow, each to be shown:
- The test reads the guard over the real home's program directories, so its result depends on
  what else runs on the machine.
- A real `takt run` would refuse a step for the same unrelated write: the guard cannot tell a
  write by the step from one by another process, and a refused step cannot be put back.

## Check

Not runnable by `scripts/check-finding` as it stands: it needs a write into a guarded program
directory while a step runs. A probe that writes a file into the directory from another process
during a `--harness none` run's `ci.read` stands in.

```expected_before
command: cargo test -p b10x-takt --test approval_riders_adv2 & p=$!; for i in $(seq 40); do cp "$(command -v true)" "$HOME/.cargo/bin/takt-probe-$i"; sleep 0.1; done; rm -f "$HOME"/.cargo/bin/takt-probe-*; wait $p
exit: 101
healthy_exit: 0
output: wrote what its validator does not
source: docs/waves/wave-39.md
```

## Outcome

A takt test's result does not depend on writes outside its own directories. A run's guard either
attributes a write to the step that made it or says the write's origin is unknown in a way the
run does not treat as the step's own.

## Acceptance

1. The tests that run steps under the guard give a home of their own, or a guard root of their
   own, so a write to the real home's program directories does not reach them.
2. The predicate above exits 0.
3. The guard's behaviour on a write it cannot attribute is decided and recorded (refuse, warn, or
   attribute by process), with a test for the choice.

## Scope

- `crates/takt/tests/approval_riders_adv2.rs` (cited)
- `crates/takt/src/run.rs`: the guard of program and import directories (inferred)
