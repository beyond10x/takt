---
title: Run a story
description: Let takt take the next ready story from the planning store and deliver it through a pull request.
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

- The repository has an AEP planning store (`.engineering/project.yaml`) with at least one ready
  story, and `aep` is on the `PATH`.
- The tree is clean: a case opens only on a clean tree.
- For the pull request: `origin` is on github.com, `b10x-gates` and `gh` are on the `PATH`, and
  `B10X_GATES_POLICY` names the gates policy. With another host, or with `--forge none`, no pull
  request is opened.
- The tests are `--test-cmd` (default `cargo test --quiet`, split at white space, run without a
  shell). `.engineering/gates.yaml`, when the repository has one, adds the gate.

## What happens

1. takt opens the case on a branch of its own and records `opened` in `events.jsonl`.
2. The harness writes the tests and the implementation, one admissible action per step; each step
   that changes files is committed.
3. The tests and the gate run as validators on the implementation revision.
4. Once both are green, `pr.open` pushes the branch and opens the pull request through the bot
   route; `ci.read` reads its checks, waiting while they are pending.
5. With CI green, `repository.merge` needs a grant. The run stops on `ApprovalRequired`, exit 4.
   See [Approve and resume](./approve-and-resume.md).
6. After the grant, the next run merges, records the evidence on the story, moves it to
   `implemented` and stops on `Completed`, exit 0. The records reach the base with the merge.

## Bounds

`--iterations` (steps), `--hours` and `--tokens` stop the run on a budget, exit 3. A run that waits
on the forge until its budget ends stops with exit 6; the next run resumes the case.

## What is not proven yet

No recorded run has yet taken a story from ready to merged with the grant as the only manual
step. Scenario 1 reached `accepted` on 2026-10-18, but with more invocations than that allows
(`docs/design/takt.md` § 4).

Sources: `takt run --help`, `README.md` § Use, `docs/design/takt.md` § 4.
