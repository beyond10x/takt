---
format: aep.planning-md/3
id: story:duplicate-key-reader-limits
kind: story
status: proposed
title: Three duplicate keys the YAML reader does not find at their line
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: duplicate-key-reader-limits
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run_config.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T06:37:36Z", actor: "human:timo", revision: 4}
---
# Three duplicate keys the YAML reader does not find at their line

## What is wrong

The duplicate-key reader of a YAML run configuration (`crates/takt/src/run_config.rs`, wave 40)
gives the generic refusal at the mapping's first line, and says nothing of a key held twice, for
three inputs the adversarial pass of wave 40 wrote (`crates/takt/tests/w40_adv_dup_and_pin.rs`,
ignored with this story's id):

- a duplicate written as an alias key: `&k iterations: 1`, then `*k : 2` (`run_config.rs:987`);
- a tagged key before the duplicate: `!!str model: x`, then `iterations` twice (`:978`);
- a plain key ending in a colon: `x:: 1`, then `x:: 2` (`plain_scalar`, `:912`).

The file is still refused in each case, so nothing unsafe runs; only the line and the key are
lost. No run configuration is known to use any of the three.

## Check

```expected_before
command: cargo test -p b10x-takt --test w40_adv_dup_and_pin -- --ignored
exit: 101
healthy_exit: 0
output: test result: FAILED
source: crates/takt/tests/w40_adv_dup_and_pin.rs
```

## Outcome

Each of the three is refused at the duplicate's line, saying a key is held twice, and naming the
key when it is a declared name.

## Acceptance

1. The three ignored cases pass with their `#[ignore]` removed.
2. Every case of `run_config_adv1.rs` and `run_config_dup_adv2.rs` still passes.

## Scope

- `crates/takt/src/run_config.rs` (cited)

## Release 0.19.0

0.19.0 ships this story's cases `#[ignore]`d, accepted at its release check as a tracked finding
on the condition that each ignore's reason names this story.
