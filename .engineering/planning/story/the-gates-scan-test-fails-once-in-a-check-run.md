---
format: aep.planning-md/3
id: story:the-gates-scan-test-fails-once-in-a-check-run
kind: story
status: draft
title: The gates-command scan test fails in the local forge's check environment
summary: harness::tests::the_gates_commands_are_named_only_when_they_pass_the_scan failed once in a local forge check run on an unrelated change
tags:
- finding
- priority:medium
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# The gates-command scan test failed once in a check run and passed on three reruns

## What is wrong

`harness::tests::the_gates_commands_are_named_only_when_they_pass_the_scan`
(`crates/takt/src/harness.rs`) fails in every local forge check run of
`cargo test --locked -p b10x-takt --lib` observed in wave 44 (two heads of one case, 209 passed,
1 failed, at `harness.rs:6094`: the block names no command, so the fake scan did not pass), and
passes outside it: three runs in the clone, with `TMPDIR` unset, and under `env -i HOME PATH`.
The check runs through `Confinement::run_test` (its environment and scratch home) in a fresh
repository under the state directory. The test writes its fake `gates` script under
`std::env::temp_dir()` and runs it; which part of the check environment stops that is not
established.

## Check

Not reproduced on demand. A loop of library runs is the instrument:

```expected_before
command: for i in $(seq 30); do cargo test --locked -p b10x-takt --lib -q >/dev/null 2>&1 || exit 101; done
exit: 101
healthy_exit: 0
output: ""
source: crates/takt/src/harness.rs
```

## Outcome

The cause is established from a failing run's output, and the test does not fail on a change that
does not touch it.

## Acceptance

1. The loop above exits 0 on three runs.
2. The cause is named in the fix's commit, from the output of a failing run.
