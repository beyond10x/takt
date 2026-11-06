---
format: aep.planning-md/3
id: story:claim-and-report-on-a-tracker
kind: story
status: draft
title: A tracker source claims an item at open and reports after the merge
summary: claim so two runs never take one item; report with the pull request
relations:
- decomposes: epic:work-sources
- serves: vision:safe-agentic-coding
- depends_on: story:work-from-jira-github-and-gitlab
revision: 1
---
# Story: claim-and-report-on-a-tracker

## Outcome

A tracker source claims an item when its case opens (a label, an assignee or a transition the run file names), so two
runs never take one item, and reports on it after the forge has merged or the run has stopped (a comment with the
pull request and the outcome, a transition on merge).

## Context

For `aep` the claim and the report are commits on the case branch that ride the pull request before the merge, checked
by `approvals_only` (`crates/takt/src/case.rs`). A tracker's claim and report are writes outside git, after the merge:
each is recorded in the case with its outcome (applied, refused, unknown), and an unknown one is read back before it is
sent again, as `pr.open` and the merge are.

## Acceptance

- A claimed item is not selected by a second run: a test against a fake endpoint.
- A report after the merge carries the pull request's URL; a lost answer is read back, never sent twice: a test.
