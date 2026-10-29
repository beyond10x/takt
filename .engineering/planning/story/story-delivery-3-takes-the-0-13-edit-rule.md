---
format: aep.planning-md/3
id: story:story-delivery-3-takes-the-0-13-edit-rule
kind: story
status: draft
title: takt's story.delivery/3 adopts the edit rule engineering-protocols 0.13.0 fixed in /2
summary: story.delivery/3 keeps 0.12.2's edit rule, so tests that passed on their first run still reach the edit through a red gate
tags:
- finding
- priority:low
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# takt's story.delivery/3 adopts the edit rule engineering-protocols 0.13.0 fixed in /2

## What is wrong

`protocols/story-delivery/3.yaml` (`implementation.edit`, lines 196-212) keeps engineering-protocols
0.12.2's rule: after a red gate the edit is admissible while the tests pass and the first
`tests_change` is not result `none`. engineering-protocols 0.13.0 calls that a defect in its
`story-delivery/2` and replaces it with a rule that a test failed after the tests were written
(Canon 0.7.0's `after` form). Under /3, tests that passed on their first run still reach the edit
through a red gate, and /3 now differs from the pinned /2 in two rules, not in the one that
`crates/takt/src/protocol.rs:29-34` describes.

Seen in wave 44's adversarial pass on the loom 0.49.1 bump (EP 0.13.0, Canon 0.7.0).

## Check

```expected_before
command: grep -q 'after:' protocols/story-delivery/3.yaml
exit: 1
healthy_exit: 0
output: ""
source: protocols/story-delivery/3.yaml
```

## Outcome

`story.delivery/3`'s `implementation.edit` takes engineering-protocols 0.13.0's rule, its
description and `protocol.rs`'s comment say so, and /3 again differs from the pinned /2 in the
handover rule alone. The rule's `after` compares whole seconds under Canon 0.7.0; takt waits for
the next second before stamping (wave 44 U4), which this story relies on.

## Acceptance

1. The predicate above exits 0.
2. A case whose tests passed on their first run does not reach `implementation.edit` through a red
   gate on story.delivery/3; a case whose tests failed after they were written does.
