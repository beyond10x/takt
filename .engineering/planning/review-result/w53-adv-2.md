---
format: aep.planning-md/3
id: review-result:w53-adv-2
kind: review-result
status: active
title: Wave 53 units d, a, b and c, adversary pass
summary: three findings, all introduced; decided, each fixed by the review fixes
relations:
- reviews: story:a-slow-runner-is-no-late-ci-result
- reviews: story:merge-is-not-sent-after-a-merged-read
- reviews: story:pr-open-cut-off-by-a-crash-is-read-back
- reviews: story:program-rewritten-through-a-link-refuses-the-step
revision: 1
---
# Adversary, wave 53 units d, a, b and c

```
unit: story:a-slow-runner-is-no-late-ci-result + story:merge-is-not-sent-after-a-merged-read + story:pr-open-cut-off-by-a-crash-is-read-back + story:program-rewritten-through-a-link-refuses-the-step
verdict: needs change
cases: executed 14, red 4
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: the adversary's run logs in the wave's scratch directory
needs-coordinator: yes, decided below
```

Tree `takt-w53-int` at `wave/53` a023aad5b, plus one untracked test file, `crates/takt/tests/w53_adversary.rs`.
The cases run one at a time: cases that write links into the binary's shared program directory while other cases
run steps reach the accepted cause of story:a-program-reinstalled-at-its-name-during-a-test-step-refuses-it.

## Findings

| # | where | verdict | origin | finding |
|---|---|---|---|---|
| F1 | `crates/takt/tests/ci_wait_backoff.rs:597` | needs change | introduced | the allowance `cap + own + max(own, 20 ms)` is at least `cap + 2·own`, so once the runner's own time between reads exceeds about one interval a run one whole cap late passes; the story's own runner (own 524 ms) is such a runner |
| F2 | `crates/takt/src/validator.rs:4384`, `crates/takt/src/forge/command.rs:880` | infeasible | introduced | the number the command forge's `find` prints is bound to the case without the read-back `open` performs, so a `find` that ignores `{branch}` or `{base}` binds the case to another branch's request, or to one into another base (two red cases) |
| F3 | `crates/takt/src/forge/local.rs:630`, `:671` | confirmed | introduced | `open_pull_request` creates `<n>.json` and writes it afterwards; a concurrent open or a crash between the two leaves an empty record, which the strict read of `find` turns into a failure of every `pr.open` on that hub |

Attacked without a break: a second read before the merge that finds the pull request merged or closed at a head that
is not takt's; `open.unanswered.json` kept and removed by outcome; GitLab's list with closed, other-base and
other-branch entries; the forms `find` may print; a chain of links, a re-pointed middle link, a relative link, a
dangling link made to resolve and a program replaced by a link (all refused).

## Decisions

Decided 2026-11-04 under the owner's standing delegation, through the orchestrator:

- F1: detection is measured apart from the write. The read that saw green is at most `cap + own` after the checks
  passed; the record's modification time after that read has its own small bound.
- F2: the found number is read back through `read` and taken only when its branch, base and open state match the
  request, as `open` checks its own number.
- F3: the cause is fixed and the strict read kept: a record is written whole to `<n>.json.<pid>.next` and hard-linked
  to `<n>.json`, which fails if the name exists; readers pass over `.next` files.
  `the_local_forge_reads_its_records_strictly` stays as it is.

The adversary's file is committed with the fixes, its four red cases green.
