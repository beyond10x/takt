---
format: aep.planning-md/3
id: story:scan-scope-test-is-not-flaky
kind: story
status: implemented
title: The scan-scope test of the failed checks' block fails now and then
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: scan-scope-test-is-not-flaky
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/harness.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T21:15:40Z", actor: "human:timo", revision: 3, executor: "agent:planner"}
- {from: "proposed", to: "active", at: "2026-10-28T21:15:41Z", actor: "human:timo", revision: 4, executor: "agent:developer"}
- {from: "active", to: "implemented", at: "2026-10-29T21:30:09Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# The scan-scope test of the failed checks' block fails now and then

## What is wrong

`harness::tests::failed_checks_pass_the_scan_scope_files_pass`
(`crates/takt/src/harness.rs:4563` at 7c9e7129) failed once in four runs of the library's tests
during wave 39 (`docs/waves/wave-39.md`, the log of U1's fix round 1): the first assertion's left
side ended in `the secret scan could not run.`, where the scan was expected to pass. The three
runs after it passed.

The test writes a fake `gates` shell script, makes it executable and runs it through
`failed_block_scanned` at once. Inferred, not shown: another test thread forks while the script's
write descriptor is open, the child holds it, and `execve` of the script fails with `ETXTBSY`
("Text file busy"), which `crate::run::scan_change` reports as a scan that could not run
(`crates/takt/src/harness.rs:1228`). The cause must be shown before it is fixed.

## Check

Not runnable by `scripts/check-finding`: the command runs cargo's library tests and not `$TAKT`.
Run it in a fresh checkout of the revision. Not run when filed: the wave's gate was building in
the tree, and nothing else was built beside it.

```expected_before
command: for i in $(seq 50); do cargo test -p b10x-takt --lib 2>&1 | grep -F 'test harness::tests::failed_checks_pass_the_scan_scope_files_pass ... FAILED' && exit 1; done; exit 0
exit: 1
healthy_exit: 0
output: failed_checks_pass_the_scan_scope_files_pass ... FAILED
source: docs/waves/wave-39.md
```

## Outcome

The test gives the same result on every run of the library's tests, whatever other tests run
beside it.

## Acceptance

1. The cause is shown before the fix: a run that reproduces the failure and names the error the
   scan's start returned, recorded in the commit.
2. The predicate above exits 0: 50 runs of `cargo test -p b10x-takt --lib`, the test never
   reported failed.
3. The test still covers what it covers: a passing scan shows the block, a refused scan and a scan
   that cannot run each give their one line, and with nothing failed no scan runs.
4. If the cause is `ETXTBSY` and other tests write and run a script the same way, they are
   changed alike, or the story names them for a follow-up.

## Scope

- `crates/takt/src/harness.rs`: `failed_checks_pass_the_scan_scope_files_pass` (cited)
