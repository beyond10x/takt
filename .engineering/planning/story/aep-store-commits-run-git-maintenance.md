---
format: aep.planning-md/3
id: story:aep-store-commits-run-git-maintenance
kind: story
status: draft
title: The aep store commits in a run start git's automatic maintenance
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: aep-store-commits-run-git-maintenance
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:gate-tests-are-deterministic
revision: 1
---
# The aep store commits in a run start git's automatic maintenance

## What is wrong

Every git takt runs itself is told `gc.auto=0` and `maintenance.auto=false`
(`NO_CONFIGURED_CODE`, `crates/takt/src/git.rs`), because a commit otherwise starts `git
maintenance run --auto --detach`, a detached process whose repack can write `.git/info/refs` while
a later step of the run is guarded. The `aep` children takt starts for the planning store
(`Aep::plan_as`, `crates/takt/src/frontier.rs`) commit to the same repository (a story's move to
active, the record a run writes) and are told neither: the git they run reads the repository's and
the user's configuration, so their commits can start that maintenance.

Wave 42 U8 (story:gate-tests-are-deterministic) found the mechanism in the tests: a test's own git
commit started `git maintenance run --auto --detach`, whose repack wrote `.git/info/refs` during a
later guarded step; two loose objects in `objects/17`, the directory git samples, are enough. U8
gave the tests `maintenance.auto=false` and `gc.auto=0` through `GIT_CONFIG_COUNT`
(`crates/takt/tests/support/own_state.rs`) and left the run's own `aep` children as they are. Not
yet seen in a run outside the tests.

## Check

A source check: `frontier.rs` sets no maintenance configuration on the `aep` child.

```expected_before
command: grep -q 'maintenance.auto' crates/takt/src/frontier.rs
exit: 1
healthy_exit: 0
source: crates/takt/tests/support/own_state.rs
```

## Outcome

Every `aep` child takt starts runs its git with `maintenance.auto=false` and `gc.auto=0`, appended
to the caller's `GIT_CONFIG_COUNT` entries as `own_state.rs` does, so a store commit in a run
starts no detached process.

## Acceptance

1. The predicate above exits 0.
2. A test makes a store commit through `Aep` in a repository whose `objects/17` holds two loose
   objects and finds no `git maintenance` child started and no `.git/info/refs` written.

## Scope

- `crates/takt/src/frontier.rs`: `Aep::plan_as` (cited)
- `crates/takt/src/git.rs`: `NO_CONFIGURED_CODE`, if the two entries are shared (inferred)
- `crates/takt/tests/`: a new test file for acceptance 2 (inferred)
