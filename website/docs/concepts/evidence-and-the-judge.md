---
title: Evidence and the judge
description: Only a validator writes evidence; evidence is bound to the revision it observed; a merge needs a grant at that revision.
---

# Evidence and the judge

## Evidence comes from validators only

A validator is a program. It turns an observation (a test run's exit status, a gate result, a pull
request's CI, a merged pull request) into a `canon-evidence/1` record bound to the revision it
observed. The model never produces evidence. Tests that passed on one revision say nothing about
the next: after a new commit the claim is UNKNOWN again, and the merge waits.

takt 0.1.0 has one validator per claim of `story.delivery/1`:

| action | validator | evidence |
|---|---|---|
| `tests.run` | the test command (`--test-cmd`, default `cargo test --quiet`) | `test_result` pass or fail |
| `gate.run` | the repository's gates (`.engineering/gates.yaml`) | `gate_result` pass or fail |
| `pr.open` | the bot route opens the pull request | `pull_request` open |
| `ci.read` | `gh` reads the pull request's checks | `ci_result` pass or fail |
| `repository.merge` | the merge through the bot route, under a grant | `merge_observation` merged |

A validator observes under the same guard as a judgement step and on a tree with nothing
uncommitted outside the case. A test command that writes a grant, a record or the gates document
is refused and its writes are undone. The gates document and the project file are read from the
case's base revision, not from the tree the harness edits.

## A merge needs a grant

The one gate written in Rust is the merge validator's: it merges only under a grant of
`repository.merge` recorded for the case's revision. `takt approve` writes that grant. Without it
the run stops on `ApprovalRequired`, exit 4.

## The judge, for takt's own changes

When takt improves takt, the judge is meant to be a bundle: a promoted takt binary that names its
revision in `--version`, and the protocol, the gates and the planning store, all read from the
promoted base revision rather than from the candidate branch. 0.1.0 has the first parts: the
binary names its revision, and `takt promote` installs a binary only when its scorecard shows no
regression. The guard that reads the protocol and the store from the base, and refuses a
judge-surface merge without the owner's grant, is not built (`story:judge-guard`).

Judge surfaces are every path except `docs/**`, `README.md` and `CHANGELOG.md`. Until the guard
lands, the rule is held by the agents and their roles ([Roles](./roles.md)).

Sources: `AGENTS.md` § Rules, `crates/takt/tests/validator_confinement.rs`,
`design:recursive-self-improvement` § The judge.
