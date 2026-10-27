---
title: Approve a capability
sidebar_position: 4
description: Grant a case the capability it stopped for with takt approve, check what was written, resume the case, and read the refusals.
lede: A run that needs a capability stops and waits; takt approve records one person's or role's grant, bound to the revision at hand, and the next run goes on under it.
source: takt approve --help and takt run --help at 0.14.0; crates/takt/src/frontier.rs (approve); runs of takt 0.14.0 on scratch repositories with --harness none and with Codex, 2026-10-26
---

# Approve a capability

Some actions of a protocol need a capability. In `software.change/1` and `story.delivery/1` the one
you meet is `repository.merge`. When it is the next thing left, the run stops:

```console
stopped: ApprovalRequired (repository.merge); steps 2, tokens 0, case ~/takt-demo/.engineering/cases/takt-20261026T234353Z
```

The exit code is 4. What a capability is, and why the model cannot grant one:
[Capabilities and approvals](../concepts/capabilities-and-approvals.md).

## 1. Grant it

```sh
takt approve --repo . takt-20261026T234353Z repository.merge --by human:demo
```

```console
approved: repository.merge for case takt-20261026T234353Z by human:demo, written to takt-20261026T234353Z/authority.yaml
recorded: nothing in the planning store; the case delivers no story (it was opened on an intent, or is an improvement turn)
```

`--by` says who grants it: `human:<name>` for a person, `agent:<role>` for an agent role.
`agent:takt`, takt itself, is refused, and so is a name without a prefix.

## 2. Check what was written

```console
$ cat .engineering/cases/takt-20261026T234353Z/authority.yaml
- capability: repository.merge
  decision: granted
  approver: human:demo
  granted_at: 2026-10-26T23:43:53Z
  revision: c9c629581b0173893efdf50e4b2fe8b7af55624e
```

`revision` is the commit the grant is bound to.

For a case that delivers a story, `takt approve` also records the approval on the story in the
planning store, as approval evidence that names the case's `authority.yaml`. It commits that
record on the case's branch as `takt: approve <story> <capability>`. For that the tree must be on
the case's branch and clean. A case opened on an intent gets the grant alone: nothing in the
store, no commit, and no store needed.

## 3. Resume

| the case | resume with |
|---|---|
| a story's | `takt run --repo .`, which resumes the open case of the story being worked |
| an intent's | `takt run --repo . --case <case>`; without a planning store, a run with neither `--intent` nor `--case` is refused |

The resumed run needs a harness. Merging is a judgement step. `--harness none` takes the action
the case is granted, once it is admissible, and no other judgement step; with `--forge none`
nothing performs that merge, so the step changes no file and the run stops:

```console
$ takt run --repo . --case takt-20261026T234353Z --harness none --forge none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (admissible), tests.run (admissible), tests.write (admissible)
step 1: repository.merge (none)
  said: no harness: the action the case is granted is taken
  effect: no file changed
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (admissible), tests.run (admissible), tests.write (admissible)
stopped: NoProgress; steps 1, tokens 0, case ~/takt-demo/.engineering/cases/takt-20261026T234353Z
```

The case stays open, exit 3. Resumed with a forge, the merge is performed by the forge's merge
validator, the case completes, and the run exits 0. Without a grant, `--harness none` requests the
approval as before and does not merge.

## A commit after the grant needs a new one

The grant holds for the revision it names. When a step commits after it, the merge needs a grant
again. In this resume of a Codex case, the model edited once more before it asked for the merge:

```console
$ takt run --repo . --case takt-20261026T233005Z --harness codex --forge none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (admissible), tests.run (admissible), tests.write (admissible)
step 1: implementation.edit (codex)
  effect: committed 1 path(s); implementation is now 718c486b
step 2: tests.run {}
  effect: the test command exited 0: pass; 2 passed, 0 failed, 0 ignored over 2 result lines
step 3: repository.merge (codex)
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 3, tokens 45753, case ~/takt-demo2/.engineering/cases/takt-20261026T233005Z
```

(The model's `said:` lines, the `evidence:` line and the later `frontier:` lines are left out.) The first `frontier:`
line shows the grant at work: `repository.merge` is `admissible`. The edit moved the revision,
so the merge went back to `approval-required`.

## When takt approve refuses

Each refusal exits 1 and writes nothing:

| the case | refusal |
|---|---|
| not stopped for that capability | `is not at the gate for repository.merge: its latest decision finds repository.merge blocked, not approval-required; run it until it stops ApprovalRequired` |
| `--by` without `human:` or `agent:` | `the approver ... is not an actor the planning store records as granting` |
| a story's, with the tree on another branch or dirty | refused before anything is written |

The full list is in
[What takt approve refuses](../concepts/capabilities-and-approvals.md#what-takt-approve-refuses).
