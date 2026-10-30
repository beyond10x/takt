---
title: Evidence and the judge
sidebar_position: 5
description: Only a validator, a program takt runs, writes evidence, and each record is about one exact commit. This page also explains whose grant a merge needs.
lede: Evidence is a canon-evidence/1 record a validator, a program, writes about the exact revision it observed; a model's text is never evidence, a new commit makes every claim about the implementation unknown again, and a change to a judge surface merges only under the owner's grant or a delegation the owner recorded.
source: AGENTS.md § Rules; crates/takt/src/validator.rs (TestsRun, GateRun, ChangeRead, PrOpen, CiRead, PrMerged, judge_guard, JUDGE_ALLOW, DELEGATIONS), evidence.rs (CaseEvidence::submit, second_for), lib.rs (now_after), improvement.rs, run.rs (CHOICE_PRODUCER); crates/takt/tests/validator_confinement.rs, judge_guard.rs, records_a_second_apart.rs; CHANGELOG.md 0.21.0; the record stamping checked against 0.21.0 source (evidence.rs second_for, submit); engineering-protocols 0.10.0 protocols/story-delivery/1.yaml; .engineering/judge-allow.yaml; a scratch run of takt 0.14.0
---

# Evidence and the judge

## Evidence comes from validators

A validator is a program takt runs. It turns an observation (a test run's exit status, a gate
result, a pull request's CI, a merged pull request) into a `canon-evidence/1` record bound to the
revision it observed, and the record reaches the [case](/docs/concepts/glossary#case) through the case's evidence port. This is
the record the tests of a one-test repository left, `evidence/0001-test-result-1.json`:

```json
{
  "format": "canon-evidence/1",
  "id": "test-result-1",
  "kind": "test_result",
  "result": "pass",
  "subject": "implementation",
  "subject_revision": "6074a8e0c3db22c5ff3f95ee8887eec6252c5aeb",
  "observed_at": "2026-10-26T23:11:24Z",
  "producer": "takt.tests-run"
}
```

`subject_revision` is the commit the tests ran at. [Canon](/docs/concepts/glossary#canon) applies the record to that revision
only, so after a new commit every claim about the implementation is unknown again: the tests,
the gate and CI are observed anew, and the merge waits for them.

From takt 0.22.0 every record's `observed_at` is written to the microsecond, and Canon 0.8.0 compares the records
of one evaluation at that precision, so a record the [protocol](/docs/concepts/glossary#protocol) orders after another one in the same
second is ordered after it without waiting. A case holding a record in whole seconds is compared in
whole seconds, and takt stamps an ordered record in the next second at once.

The model never produces an observation. A step that records a choice (`change.handover`: the
[harness](/docs/concepts/glossary#harness) hands the change over, changing no file) gets a record from the producer `takt.choice`.
Such a record may gate an action, never decide an outcome or an obligation alone.

## The validators

For `story.delivery/1` and `software.change/1`:

| action | what it runs | evidence |
|---|---|---|
| `tests.run` | the test command (`--test-cmd`, default `cargo test --quiet`) | `test_result` pass or fail |
| `gate.run` | the repository's gates (`.engineering/gates.yaml`) through the assertions library | `gate_result` pass or fail |
| `change.read` | git: would merging the revision into the base tip change it outside `.engineering/`? | `change_record` |
| `pr.open` | the [forge](/docs/concepts/glossary#forge) opens the pull request | `pull_request` |
| `ci.read` | the forge reads the pull request's checks | `ci_result` pass or fail |
| `repository.merge` | the forge merges, under a [grant](/docs/concepts/glossary#grant) | `merge_observation` |

The forge is an adapter ([The forge](../reference/configuration.md#the-forge)): `glab` on a GitLab
host, the run file's `forge_commands`, a Connectors instance, a bare repository on this machine
(`--forge local`), or none (`--forge none`), when the last three have no
performer. A failing `tests.run` names up to 64 failed tests (cargo, Python unittest, pytest)
and cargo's rerun hint in its summary; the [two test stops](./how-a-run-ends.md#the-two-test-stops)
read those names.

For an improvement turn (`self.improvement/1`) takt has validators of its own:
`finding.file`, `scenario.reproduce`, `scenario.verify`, `scorecard.compare`,
`candidate.merge` and `binary.promote`.

### What a validator may not do

A validator observes under the same guard as a judgement step, on a tree with nothing
uncommitted outside the case. A test command that writes a grant, a record or the gates document,
or any file its validator does not write, is refused and its writes are undone. A real refusal,
from a repository whose `Cargo.lock` was not committed, so `cargo test` wrote one, and the refusal names the three ways out:

```console
step 1: tests.run {}
  refused: tests.run wrote what its validator does not: Cargo.lock; commit Cargo.lock, or ignore it (.gitignore), or give --test-cmd a command that does not write it; the cases directory was put back, the change discarded and nothing recorded
stopped: StepRefused (tests.run wrote what its validator does not: Cargo.lock; commit Cargo.lock, or ignore it (.gitignore), or give --test-cmd a command that does not write it); steps 1, tokens 0, case ~/demo/.takt/cases/takt-20261026T231059Z
```

The gates document and the assertions project file are read from the case's base revision, not
from the tree the harness edits, so a step cannot change what judges it.

## A merge needs a grant

The merge validator is the one check written in Rust: it merges only under a grant of
`repository.merge` recorded for the revision the case reached the gate at, and it checks that
grant again itself, after Canon admitted the step. `takt approve` writes the grant; without one
the run stops `ApprovalRequired`, exit 4. The owner can also delegate the grant to the role
`agent:takt-delivery` in `.engineering/delegations.yaml`. A run then grants the merge itself, as
that role, once `tests.pass`, `gate.green` and `ci.green` hold at the revision; the merge
validator checks that grant as it checks a person's. How grants work, and the delegated grant:
[Capabilities and approvals](./capabilities-and-approvals.md#a-grant-under-the-owners-delegation).

## The judge: whose grant merges a judge surface

Before it merges, the merge validator decides whose grant the change needs. It reads two files
at the merge target's tip as the forge reports it, never from the case's tree:

- `.engineering/judge-allow.yaml`, a list of path globs that are not judge surfaces. Every
  other path is one, and so is any path that is a link or a submodule, whatever its name.
- `.engineering/delegations.yaml`, the owner's delegations to agent roles, each revocable: of
  `repository.merge.judge_surface`, the judge-surface grant, and of `repository.merge` itself,
  under which a run grants the merge as `agent:takt-delivery`.

A change that touches a judge surface merges under a person's grant (`human:<id>`), or under an
agent role's grant (`agent:<role>`) only while the delegation file names an unrevoked delegation
of `repository.merge.judge_surface` to that role. A change to either of the two files merges
under the owner's grant alone. In takt's own repository the allow-list is `docs/**`,
`README.md` and `CHANGELOG.md`, so every other path is a judge surface.

### The judge, for takt's own changes

When takt improves takt, the judge is designed as a bundle: a promoted takt binary, and the
protocol, the gates and the planning store read from the promoted base. What is built:

- the binary names its revision in `takt --version`, and `takt promote` installs a binary only
  when its scorecard shows no regression;
- the judge guard above;
- a `self.improvement/1` turn is judged by one binary: the turn records the sha256 of the
  promoted binary that opens it and refuses every resume by another.

:::caution[Planned]
Reading the protocol and the planning store from the promoted base, rather than from the case's
tree, is not built.
:::

The roles that grant and review: [Roles](./roles.md).
