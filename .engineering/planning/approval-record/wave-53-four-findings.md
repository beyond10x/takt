---
format: aep.planning-md/3
id: approval-record:wave-53-four-findings
kind: approval-record
status: approved
title: 'Wave 53: four finding stories'
summary: one wave of four finding stories for 0.31.0
relations:
- decides: story:merge-is-not-sent-after-a-merged-read
- decides: story:pr-open-cut-off-by-a-crash-is-read-back
- decides: story:program-rewritten-through-a-link-refuses-the-step
- decides: story:a-slow-runner-is-no-late-ci-result
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-11-04T06:19:34Z", actor: "human:timo", revision: 3}
---
# Approval: wave 53

## Decision

The owner's standing delegation, 2026-11-04, through the orchestrator: takt has no planned wave
left, and once every planned wave is released findings may enter. Wave 53 is approved on the same
grounds as wave 52, drawn from the remaining finding drafts, with up to four units on surfaces that
do not collide; a draft whose body is too thin to implement stays out.

The wave is four units, run one after the other in one tree because one tree builds at a time:

- unit a: story:merge-is-not-sent-after-a-merged-read, the read before the merge in
  `crates/takt/src/validator.rs`.
- unit b: story:pr-open-cut-off-by-a-crash-is-read-back, `find_open_pull_request` in the
  command, GitLab and local forges.
- unit c: story:program-rewritten-through-a-link-refuses-the-step, the program directory snapshot
  in `crates/takt/src/run.rs` and the test support's private program directory.
- unit d: story:a-slow-runner-is-no-late-ci-result, the slack of one case in
  `crates/takt/tests/ci_wait_backoff.rs`.

All four are accepted by this record, not by the agents that filed them. Units a and b both touch
`validator.rs`, a in the merge observation and b, if at all, in `pr.open`; the two functions share
no lines. The citation table in `ess/domains/` is re-pointed after each unit.

story:the-token-rule-judges-request-counts stays a draft: its body states no request tolerance and
no bound for "a minority of cases", and it changes the saved scorecard's rule, so it is too thin to
implement as written.

## Release

0.31.0: unit b may give the command forge an optional `find` command, an addition to its
configuration; the other units fix behaviour without changing a contract.
