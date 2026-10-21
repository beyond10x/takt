---
format: aep.planning-md/3
id: story:darwin-job-has-flock
kind: story
status: draft
title: The darwin job has flock
summary: brew install flock in the darwin job's setup; file-finding refuses to file without it
tags:
- finding
- macos
refs:
- provider: finding
  reference: darwin-job-has-flock
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# The darwin job has flock

## What is wrong

`scripts/file-finding` takes a lock with `flock` around the check that a finding key is free and
the write that files it, and refuses to file without it: `flock is missing, and without it a second
filing of finding:<key> could pass the check; nothing was written`. macOS ships no `flock`, and the
darwin job (`.github/workflows/darwin.yml`) installs none.

Measured on Linux at 22476ae with every program on `PATH` but `flock`: of the 48 tests in
`crates/takt-cli/tests/finding_scripts.rs`, `finding_scripts_adv1.rs` and
`finding_scripts_adv2.rs`, 45 fail, each with `flock is missing`. On macos-15 these tests stopped
earlier, at `realpath -e`, until 22476ae.

## Change

The darwin job's setup installs `flock` with `brew install flock`, before the tests run. The
scripts' locking is unchanged.

## Scope

- `.github/workflows/darwin.yml`

## Check

`scripts/check-finding` cannot run this predicate: it runs on Linux, where `flock` is installed. The
machine that checks it is the darwin job on macos-15.

```expected_before
host: macos-15, .github/workflows/darwin.yml job test-cli
command: cargo test -p b10x-takt-cli --test finding_scripts
exit: 101
healthy_exit: 0
output: flock is missing
source: .github/workflows/darwin.yml
```

`healthy_exit` holds only for the tests that need nothing else macOS lacks: a test that runs
`scripts/check-finding` also needs bubblewrap (story:takt-confines-on-macos, held).
