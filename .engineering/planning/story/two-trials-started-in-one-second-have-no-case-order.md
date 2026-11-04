---
format: aep.planning-md/3
id: story:two-trials-started-in-one-second-have-no-case-order
kind: story
status: active
title: Two trials started in one second have no case order
summary: unique_trial numbers a trial as second x 1e8 plus a random nonce, so two trials in one second sort at random and decline_adv1.rs:775 fails about half the time on a fast runner.
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/decline_adv1.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T02:39:11Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-11-04T02:39:11Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
---
# Two trials started in one second have no case order

## What is wrong

Found by the `test-cli` job of the store-only pull request after wave 50. The case
`passing_over_a_declined_case_does_not_pass_over_its_storys_other_open_case`
(`crates/takt-cli/tests/decline_adv1.rs:775`) starts two scenario trials one after the other and
asserts that the first one's case id sorts before the second's. `unique_trial`
(`crates/takt-cli/src/main.rs:3829`) numbers a trial as its start second times 100,000,000 plus a
random nonce read from `/dev/urandom`. Two trials started within the same whole second are
therefore in random order, and on a fast runner the case fails about half the time:

```text
panicked at crates/takt-cli/tests/decline_adv1.rs:775:5:
the declined case is first in case order
```

The same test passed on `main` at the same code, and the code that numbers trials did not change
in the release before it.

## Check

```expected_before
command: cargo test --locked -p b10x-takt-cli --test decline_adv1 passing_over_a_declined_case_does_not_pass_over_its_storys_other_open_case
exit: 101
healthy_exit: 0
output: the declined case is first in case order
source: test-cli job of the store-only pull request after wave 50, first attempt
```

## Outcome

The case does not depend on the order of two trial ids taken in one second: either it names
which trial is first from the run it made, or trials started in one second are numbered in the
order they start. Run in a loop, the case stays green.

## Scope

- `crates/takt-cli/tests/decline_adv1.rs`: the order assertion (cited)
- `crates/takt-cli/src/main.rs`: `unique_trial`, if trials are to be numbered in start order (cited)
