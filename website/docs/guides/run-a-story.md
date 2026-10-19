---
title: Run a story
description: Let takt take the next ready story from the planning store, deliver it through a pull request, and merge it under a grant.
---

# Run a story

```sh
takt run --repo <path> --hours 1
```

With no `--intent` and no `--case`, takt resumes the open case of a story being worked, or takes
the next ready story from the repository's planning store and opens its case on
`story.delivery/1`. A story is ready when it is `proposed`, nothing blocks it and its dependencies
are done.

## Before the first run

- What [Getting started](../getting-started.md#what-it-needs) lists for story runs and pull
  requests.
- The tree is clean.
- The tests are `--test-cmd` (default `cargo test --quiet`, split at white space, run without a
  shell). `.engineering/gates.yaml`, when the repository has one, adds the gate.

## What happens

1. takt opens the case on a branch of its own and records `opened` in `events.jsonl`.
2. The harness writes the tests and the implementation, one admissible action per step; each step
   that changes files is committed.
3. The tests and the gate run as validators on the implementation revision.
4. Once both are green, `pr.open` pushes the branch and opens the pull request through the bot
   route; `ci.read` reads its checks, waiting while they are pending.
5. With CI green, `repository.merge` needs a grant. The run stops on `ApprovalRequired`, exit 4
   ([Approve and resume](#approve-and-resume)).
6. After the grant, the next run commits the evidence and the move to `implemented` on the case's
   branch, then merges, so both reach the base with the merge; it stops on `Completed`, exit 0.

## Approve and resume

```sh
takt approve --repo <path> <case> repository.merge --by <who>
takt run --repo <path>
```

`takt approve` writes the grant into the case's `authority.yaml` and records the approval on the
case's story in the planning store; that record is committed on the case's branch, so the tree
must be on that branch and clean. `--by` names who grants. The next `takt run` resumes the case
and merges under the grant.

A grant binds to the revision of the artifact its action is about, so a commit after the grant
needs a new one. Above the granted revision, only takt's records for the story may ride to the
base: one approval per grant, one `test_result` and the move to `implemented`.

## Bounds and resuming after a stop

`--iterations` (steps), `--hours` and `--tokens` bound the run.

| stop | exit | what the next run does |
|---|---|---|
| `ApprovalRequired` | 4 | merges under the grant once one is recorded |
| `Waiting` on the forge until the budget ended | 6 | reads the forge again |
| an error once the case was open | 5 | resumes the open case |
| a budget, `NothingAdmissible`, `StepRefused`, `NoProgress` | 3 | resumes the open case of the story |

`--case <id>` resumes a named case instead.

## What is not proven yet

No recorded run has yet taken a story from ready to merged with the grant as the only manual
step. Scenario 1 reached `accepted` on 2026-10-18, but with 4 invocations where 2 were required
(`docs/design/takt.md` § 4).

Sources: `takt run --help`, `takt approve --help`, `crates/takt/src/case.rs` (`approvals_only`),
`crates/takt/src/frontier.rs`, `docs/design/takt.md` § 4, the stories `authority-hardening`,
`grant-binds-to-action-subject` and `approval-riders-are-grants`.
