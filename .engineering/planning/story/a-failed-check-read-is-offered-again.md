---
format: aep.planning-md/3
id: story:a-failed-check-read-is-offered-again
kind: story
status: draft
title: A failed check read at a revision is offered again, so a check that failed once does not end the case
summary: after a failing ci.read the harness is offered only edits, so a check that failed once or a changed check list ends the case Declined until a commit
tags:
- finding
- priority:high
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A failed check read at a revision is offered again, so a check that failed once does not end the case

## What is wrong

After `ci.read` records a failing `ci_result` at a revision, a judgement step is offered only the
edits; `ci.read` is not offered again at that revision. When nothing in the change caused the
failure (a check that failed once and passes when rerun, or a check list the operator changed),
the harness has no admissible action that reaches `ci.green`, declines, and the case stays
`Declined` until a commit moves its revision.

Seen in wave 44, twice in three runs on the local forge:

- One run's checks included 20 cases that fail in takt's test environment
  (story:promote-cases-skip-without-an-adapter); after the operator narrowed `local_checks`, the
  declined case took no step until an empty commit moved it.
- Another run's checks failed once on
  `harness::tests::the_gates_commands_are_named_only_when_they_pass_the_scan`, which passed three
  reruns at the same head; the case declined, and only an empty commit moved it.

## Check

Not runnable by `scripts/check-finding`: it needs a recorded case. On the wave-44 case of
story:docs-edits-the-tests-read-are-attempts, the stop after the failing read is
`Declined (repository.merge requires ci.green; the offered edits do not resolve the missing CI
evidence.)`.

```expected_before
command: grep -c '"decline"' .takt/cases/story-docs-edits-the-tests-read-are-attempts-20261029T070534Z/events.jsonl
exit: 0
healthy_exit: 1
output: "1"
source: a recorded wave-44 case
```

## Outcome

A failing check result at a revision leaves `ci.read` admissible at that revision a bounded number
of times (or once the check list's digest changed), so the harness can read the checks again
instead of declining; a read that fails the same way again is final, as today, and the case then
needs an edit.

## Acceptance

1. A scripted case whose first `ci.read` fails and second passes at the same revision reaches
   `repository.merge` without a commit.
2. A case whose `ci.read` fails the same way twice at a revision is offered only the edits, as
   today.
