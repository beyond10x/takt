---
format: aep.planning-md/3
id: story:approval-tests-pass-on-a-cold-cache
kind: story
status: draft
title: The approval tests pass on a machine whose aep protocol cache is empty
tags:
- finding
- priority:medium
refs:
- provider: finding
  reference: approval-tests-pass-on-a-cold-cache
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 3
---
## Outcome

`task check` passes on a fresh machine without a warm-up: the tests that run `aep` do not race to fetch the same protocol snapshot into an empty `~/.cache/aep/protocol-sources`, either because each test gets its own cache directory or because the cache is filled once before they start.

## Context

From the release CI fix of 2026-10-19 (fix/check-installs-aep, merged at 6ba6e1c): with `aep` installed and a fresh `HOME`, the `approval_record` tests failed 4 of 6 and 0 of 6 in two cold runs and passed 6 of 6 warm. `.github/workflows/check.yml` works around it with a step that runs `aep plan artifact validate` before the tests; that step also gates store validity on every pull request and on the release gate (grant review of fb940e1). A developer with an empty cache still hits the race.

## Scope

- `crates/takt/tests/approval_record*.rs`: the aep calls and their environment (inferred)
- `.github/workflows/check.yml`: the warm-up step, removed once the tests hold without it (cited)

## Acceptance

With `HOME` set to an empty directory and `aep` on `PATH`, `cargo test -p b10x-takt --test approval_record` passes 5 of 5 runs without a prior `aep` command.

## Seen again in wave 42 (U8)

Wave 42 U8 (story:gate-tests-are-deterministic) found `approval_record` failing 6 of 6 under an
empty `XDG_CACHE_HOME`: `aep` fetched the fixture's pinned protocol revision into the empty cache,
and the fetch failed in the tests' environment ("fetching the pinned protocol revision: …
Permission denied (publickey)"). U8 pointed the tests at `$HOME/.cache/aep`
(`crates/takt/tests/support/own_state.rs`), the cache the CI step fills before the tests, which
hides the cold cache rather than fixing it. With an empty cache named through `AEP_CACHE_DIR` the
six tests still fail: on the wave 42 tree at a2f650bf, 3 runs of 3, each 0 passed and 6 failed,
every one at `story:ready-one is taken` (`crates/takt/tests/approval_record.rs:228`).

## Check

Not runnable by `scripts/check-finding`: it runs a library test, not `$TAKT`.

```expected_before
command: AEP_CACHE_DIR="$(mktemp -d)" cargo test -p b10x-takt --test approval_record
exit: 101
healthy_exit: 0
output: story:ready-one is taken
source: crates/takt/tests/approval_record.rs
```

The Acceptance above holds when this predicate exits 0 on a machine with no SSH key for the
protocol source's host, with no prior `aep` command.
