---
format: aep.planning-md/3
id: story:an-open-pull-request-is-found-on-resume
kind: story
status: draft
title: A resumed run finds the pull request already open for its branch
summary: every forge adapter implements find_open_pull_request; 0.24.0 regression
tags:
- finding
- priority:high
- trial-0.24.0
relations:
- informed_by: story:pr-open-not-idempotent-on-resume
revision: 1
---
## Outcome

A run resumed after a lost `pr.open` answer finds the pull request the forge already holds for
the case's branch and base, and takes it instead of opening a second one or failing on every
resume. Every forge adapter implements the lookup.

## Context

Regression in 0.24.0, of story:pr-open-not-idempotent-on-resume, which is marked implemented and
no longer holds. `Forge::find_open_pull_request` has only its trait default, which answers none
(`crates/takt/src/forge.rs:384-392` at 0.24.0). No adapter overrides it: `forge.rs:1781`,
`forge/command.rs:654`, `forge/connectors.rs:1419`, `forge/gitlab.rs:625`, `forge/local.rs:548`.
The only implementations, the removed delivery route's and its shared wrapper's, went out in
0.24.0. `pr.open` (`crates/takt/src/validator.rs:4220-4284`) still relies on the lookup before and
after a failed open, and its comment (`validator.rs:4224-4228`) still promises the guard, while
`branch.yaml` is written only at the end (`validator.rs:4284`).

Inferred consequences, from the 0.24.0 trial cycle's run-loop review:
- a run killed between the open and `branch.yaml` opens a second request on resume (the local
  forge hands out the next number);
- an open whose answer was lost fails on github (422 "already exists") or gitlab (409), the
  lookup answers none, and the error stands on every resume.

```expected_before
command: git grep -c 'fn find_open_pull_request' -- crates/takt/src
output: "crates/takt/src/forge.rs:1"
```

## Acceptance

1. The local, gitlab, command and connectors (github and gitlab) adapters each answer
   `find_open_pull_request` with the open request for the branch and base, and none only when the
   forge reads none: local from `requests/*.json`; gitlab by source and target branch; github
   by `GET /pulls?head=&base=`.
2. Per adapter, a conformance test opens across a lost answer, resumes, and sees exactly one
   open request (one POST), recorded in `branch.yaml`.
3. An answer the adapter cannot read is an error, never none.
4. story:pr-open-not-idempotent-on-resume's acceptance holds again on 0.25.x, and that story
   names this one.
