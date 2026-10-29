---
format: aep.planning-md/3
id: story:scenario-corpus-realistic
kind: story
status: draft
title: The held-out checks of scenarios 1 to 6 and their controls are committed
summary: multi-file feature, hidden-test bug fix, behaviour-keeping refactor, gate-needing change, a task to refuse
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:baseline-runner-in-repo
scope:
- confidence: inferred
  path: crates/takt-bench/src/controls.rs
- confidence: inferred
  path: crates/takt-bench/src/main.rs
- confidence: inferred
  path: crates/takt-bench/tests/controls.rs
- confidence: inferred
  path: docs/metrics/scenarios
revision: 14
---
# Story: scenario-corpus-realistic

## Outcome

The held-out checks of scenarios 1 to 6, and the controls that show each check can tell a right result from a wrong one, are committed in this repository under `docs/metrics/scenarios/`, and `takt-bench controls`, a subcommand of the runner binary story:baseline-runner-in-repo creates, runs the controls. Anyone can then confirm that a check is able to fail.

- For scenarios 1 to 5 the right result is a committed reference patch on the seed, and the wrong result is the seed itself.
- Scenario 6 is a task the rules forbid, so it is inverted: the right result is the untouched seed with no pull request, and the wrong results are committed obedient variants that do the forbidden change.

## Context

The owner's request of 2026-10-18 was more realistic use cases, with our own benchmark first. Scenario 1 could not tell the arms apart: every arm passed its held-out check 3 of 3 (docs/metrics/README.md).

Scenarios 1 to 6 are declared in the scratch repository's `.engineering/scenarios.conf` on its main at `19f2c3d` (https://github.com/beyond10x/todo-app-example), one `scenario.<id>.seed` each, read as `takt scenario run` reads it (`Scenario::read`, `crates/takt-cli/src/main.rs:3267-3356`, over `crates/takt/src/git.rs:818-834`). Where each seed is:

- Scenario 1's seed is `865eaa8`, main's first commit, which is reachable from `19f2c3d` itself.
- The seeds of 2 to 6 were cut from a base that exists only on the `scenario/<id>` branches, so they are reachable once those branches are fetched (review-result:scenario-corpus-round-0).

`takt-bench controls` takes every seed from that file at the clone's checked-out revision, never from a branch name. The scratch repository's main has since moved on and re-declares newer seeds; the controls are pinned to `19f2c3d`, the revision the reference patches were cut against. Their acceptance scripts, reference fixes and obedient variants were never committed, though. They exist only in the reviewer's scratch directory, so nobody else can confirm that a check fails on a wrong result.

The acceptance scripts of 2026-10-18 (`acceptance-<id>.sh`) are shell. Running code committed to a beyond10x repository is Rust with clap derive, so a check is committed as data that the Rust subcommand applies and runs: `heldout.patch`, the held-out tests added to the seed, and `check.toml`, the scratch repository's own test command over them. The held-out tests run inside the scratch repository, never in takt, and are committed only inside patches, as the reference fixes are.

A control holds for a scenario when its held-out check passes on the right result and fails on every wrong one. "Passes on a reference and fails on the seed" does not fit scenario 6: for a task that should be refused, the seed is the right result, so that scenario names its wrong results explicitly, and its check takes `none`, meaning no pull request, as its input (review-result:scenario-corpus-round-0, three obedient variants).

The checks stay held out from the arms. They live in this repository, and no arm is given this repository. An arm with network access could still read them, since the repository is public. A transcript that reads them is a finding.

This story does not edit `docs/metrics/README.md`; story:baseline-runner-in-repo owns that file. Its own documentation is `docs/metrics/scenarios/README.md`.

The four-arm round on scenarios 2 to 6 is live, so it is a story of its own: story:scenario-corpus-four-arm-round. This story uses story:scenario-corpus's `takt scenario run`.

## Ordering

It adds a subcommand to the crate story:baseline-runner-in-repo creates and lands on that crate's `src/main.rs`, so it follows that story (`depends_on`). story:bench-free-arms registers `free-arm` in the same `src/main.rs`; each subcommand is a module of its own, so the two share only the subcommand list, and `aep plan artifact waves` keeps them in different waves.

## Expected before

On takt at 338082d, holds; after the change, fails:

- `ls docs/metrics/scenarios` exits non-zero.

## Scope

- `crates/takt-bench/src/controls.rs`: the `controls` subcommand (inferred)
- `crates/takt-bench/src/main.rs`: registering `controls` (inferred)
- `crates/takt-bench/tests/controls.rs`: the offline test (inferred)
- `docs/metrics/scenarios/README.md` (inferred)
- `docs/metrics/scenarios/<id>/check.toml`, `heldout.patch`, `reference.patch` (inferred)
- `docs/metrics/scenarios/6/obedient-*.patch` (inferred)

## Acceptance

1. On a clone of https://github.com/beyond10x/todo-app-example checked out at `19f2c3d`, with its `scenario/2` to `scenario/6` branches fetched, `takt-bench controls --scenarios docs/metrics/scenarios --scratch <clone>` exits 0 and prints `controls held: 6 of 6`. Each scenario's seed is the one `.engineering/scenarios.conf` at `19f2c3d` declares, scenario 1's `865eaa8`.
2. In `crates/takt-bench/tests/controls.rs`, offline, on a fixture scratch repository with one scenario whose held-out check also passes on its wrong result, `takt-bench controls` exits non-zero, names the scenario and the wrong result, and prints `controls held: 0 of 1`.

Before the change `docs/metrics/scenarios/` does not exist.
