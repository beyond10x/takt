---
format: aep.planning-md/3
id: story:ci-runs-the-real-harness-cases
kind: story
status: draft
title: CI runs the four cases that need a real b10x-harness
summary: a pinned, checksum-verified harness release asset in test-cli; the four 0.25.0 ignores removed
tags:
- known-limit
- priority:high
- release-0.25.0
relations:
- informed_by: story:git-and-model-through-loopback-relays
revision: 1
---
## Outcome

CI installs a pinned, checksum-verified b10x-harness release asset and runs the four cases that
need a real harness binary, so a keyless loopback harness session is proven on every pull
request instead of only by hand.

## Context

0.25.0 added four `#[ignore = "needs a real b10x-harness on the program path or at
TAKT_TEST_B10X_HARNESS"]` cases. CI runs none of them:
- `crates/takt-cli/tests/loopback_relays_cli.rs:781`,
  `the_real_harness_sends_the_stand_in_no_credential_header`;
- `crates/takt-cli/tests/w48_adv_a.rs:525`,
  `the_default_token_file_in_home_is_not_sent_to_a_keyless_loopback_endpoint`;
- `w48_adv_a.rs:595`, `a_loopback_redirect_does_not_carry_the_token_to_another_host`;
- `w48_adv_a.rs:621`, `a_keyless_loopback_redirect_carries_no_credential_header_to_another_host`.

They passed once locally at 0.25.0's tree (5bcb3538) with b10x-harness 0.24.0's x86_64 Linux
release asset, verified against its SHA256SUMS. The run was
`cargo test -p b10x-takt-cli --test loopback_relays_cli --test w48_adv_a -- --ignored`, and it
reported 1 + 3 passed.

```expected_before
command: git grep -c 'needs a real b10x-harness' -- crates/takt-cli/tests
output: "crates/takt-cli/tests/loopback_relays_cli.rs:1\ncrates/takt-cli/tests/w48_adv_a.rs:3"
```

## Acceptance

1. The test-cli job downloads a pinned b10x-harness release asset, checks it against the
   release's SHA256SUMS, and sets `TAKT_TEST_B10X_HARNESS` to it.
2. The four cases run in CI and are no longer ignored. Under `CI=true`, a missing harness fails
   the job instead of skipping.
3. The pin moves with the harness version takt's Loom pairs with, and is named in one place.
