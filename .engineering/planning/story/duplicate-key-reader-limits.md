---
format: aep.planning-md/3
id: story:duplicate-key-reader-limits
kind: story
status: implemented
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
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T06:37:36Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-29T06:59:56Z", actor: "human:timo", revision: 5, executor: "agent:takt"}
- {from: "active", to: "implemented", at: "2026-10-29T08:46:45Z", actor: "agent:takt", revision: 9, decided_on: {"recorded":{"test_result":1,"approval":1}}}
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

## Decision (wave 44, review)

The second adversarial pass (on 0170e66f) found seven cases red, now in
`crates/takt/tests/w44_adv_dup_keys_b.rs`; all seven come from the lines the first fix added. The
planner's decision on how the reader holds them:

- A line is counted from a running byte cursor over the text (libyaml's mark index counts
  bytes), with `\r\n`, `\r` and `\n` each one break, never by walking the text from its start for
  each event: the reader is linear in the file.
- A tagged key's value is built from the event's resolved tag, not from a slice of the source
  text: the core tags (`int`, `float`, `bool`, `null`, `str`) by serde's rules, a local tag (`!x`)
  as a tagged value around the plain-resolved text, any other tag as a string.
- A key the reader cannot resolve stops it, as it did before the first fix: it gives up and the
  generic refusal stands, rather than reporting a later pair.

## Decision (wave 44, third review)

The third adversarial pass (on 71c38d32) found three cases red, now in
`crates/takt/tests/w44_adv_dup_keys_c.rs`; each breaks a rule of the decision above as written.
The planner's decision, which keeps that decision and settles how it reads an alias:

- A key with a local tag (`!x iterations`) is a tagged value around its plain-resolved text, so
  it never equals the untagged key (`iterations`).
- A plain key with a global tag that is not a core tag (`!<tag:example.com,2000:x> 1`) is the
  string of its text, never a number or a boolean.
- A key the reader cannot resolve stops it, and the generic refusal stands. An alias whose anchor
  is, at that point of the file, a mapping or a sequence is not such a key: a collection never
  equals a scalar key, so the reader goes on past it, as
  `adv_w44_u3_anchor_redefined_on_a_mapping_is_not_the_old_scalar`,
  `adv_w44_u3_anchor_redefined_on_a_sequence_is_not_the_old_scalar` and
  `adv_w44_u3b_alias_of_a_mapping_as_a_key_before_the_duplicate` hold. A key the reader skips
  this way and a later key that serde finds equal to it give the generic refusal.
- Keys `-0.0` and `0.0` (serde names `-0.0`, the reader prints `0.0`) give the generic refusal;
  that stands, out of this story's scope.
