---
format: aep.planning-md/3
id: story:test-aep-cache-is-the-operators-cache
kind: story
status: draft
title: The tests' aep cache is the operator's shared cache
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: test-aep-cache-is-the-operators-cache
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:gate-tests-are-deterministic
- depends_on: story:approval-tests-pass-on-a-cold-cache
revision: 1
---
# The tests' aep cache is the operator's shared cache

## What is wrong

`crates/takt/tests/support/own_state.rs` (`before_main`, line 55) sets `AEP_CACHE_DIR` to
`$HOME/.cache/aep` when the caller names none, so that a caller's `XDG_CACHE_HOME` does not move
the store tests to a cache without their fixtures' pinned protocol revision (wave 42 U8). With
`HOME` the operator's (story:test-home-is-the-operators-home), that is the operator's own aep
cache, which every test binary and every `aep` the operator runs read and fill at once. A test's
result then depends on what that cache holds, and the tests write into it.

A judgement note from wave 42's adversarial pass, not a seen failure. Giving the tests a cache of
their own makes it cold on every run, which story:approval-tests-pass-on-a-cold-cache must fix
first.

## Check

A source check, not a behaviour check: the cache is derived from the inherited `HOME`.

```expected_before
command: grep -qF 'home.join(".cache/aep")' crates/takt/tests/support/own_state.rs
exit: 0
healthy_exit: 1
source: crates/takt/tests/support/own_state.rs
```

## Outcome

A test binary uses an aep cache of its own under its private root, filled from what the tests
pin without a network fetch, or a read-only copy of a cache prepared before the tests; it writes
nothing into the operator's cache.

## Acceptance

1. The predicate above exits 0.
2. With `AEP_CACHE_DIR` unset and the operator's `~/.cache/aep` made read-only, the store tests
   (`approval_record`, `gate_tests_have_their_own_state`) pass.

## Scope

- `crates/takt/tests/support/own_state.rs`: `before_main` (cited)
- `crates/takt/tests/gate_tests_have_their_own_state.rs`: the aep cache cases (inferred)
- `.github/workflows/check.yml`: the step that fills aep's protocol cache (inferred)
