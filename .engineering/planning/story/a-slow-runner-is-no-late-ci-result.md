---
format: aep.planning-md/3
id: story:a-slow-runner-is-no-late-ci-result
kind: story
status: draft
title: A slow runner is no late CI result
summary: The one-cap CI result case allows 20 ms beyond the runner's own time; a slow darwin runner measured 863 ms against a 769 ms allowance and turned it red.
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# The one-cap CI result case allows a slow runner too little slack

## What is wrong

Found by the darwin CI job of wave 50. `ci_result_follows_any_completion_within_one_cap`
(`crates/takt/tests/ci_wait_backoff.rs:553`) checks that the `ci_result` record is written within
one cap after the checks turn green. It allows the cap, the run's own time between reads, and a
fixed 20 ms (`ci_wait_backoff.rs:595`). On a slow darwin runner the record came 863 ms after the
checks passed, while the allowance was 225 ms + 524 ms + 20 ms = 769 ms. The case went red, and it
passed on Linux at the same head:

```text
panicked at crates/takt/tests/ci_wait_backoff.rs:599:5:
ci_result later than one cap 225ms after the checks passed, as (ms after the first read, how late,
the run's own time between reads, the waits): [(1100, 862.885269ms, 524.328167ms, [0.1, 0.15, 0.225])]
```

The 20 ms covers the write of the record and the read of its modification time. A loaded runner
takes longer than that, and the case then reports a delay the run did not cause.

## Check

```expected_before
command: cargo test --locked -p b10x-takt --test ci_wait_backoff ci_result_follows_any_completion_within_one_cap
exit: 101
healthy_exit: 0
output: ci_result later than one cap
source: wave 50 darwin test-library job, first attempt
```

## Outcome

The case tells a run that waited too long from a runner that was slow. Its slack scales with the
runner's measured time, or the time it measures excludes the record's write. A loaded runner does
not turn it red, and a run that waits a whole interval too long still does.

## Scope

- `crates/takt/tests/ci_wait_backoff.rs`: the slack in `ci_result_follows_any_completion_within_one_cap` (cited)
