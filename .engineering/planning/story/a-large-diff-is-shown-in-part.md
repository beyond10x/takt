---
format: aep.planning-md/3
id: story:a-large-diff-is-shown-in-part
kind: story
status: draft
title: A diff over the prompt limit is shown in part, not withheld
summary: a diffstat and a bounded part of the diff replace the not-shown line
tags:
- trial-0.24.0
relations:
- informed_by: story:handover-costs-a-harness-call-of-its-own
revision: 1
---
## Outcome

A case diff over the prompt limit is shown as a diffstat and a bounded part of the diff, not
withheld whole, so the judgement and handover steps do not explore the repository to recover it.

## Context

A diff over 8,192 bytes (`CHANGE_SHOWN_BYTES`, `crates/takt/src/harness.rs:1647`) is replaced in
the prompt by "not shown. Read the files for it." (`TooLarge`, `harness.rs:1778`). In a cluster
trial run, change.handover then spent 27 tool calls and 459.9k of the run's 870.2k harness tokens
(53%) reading the files the diff would have shown.

## Acceptance

1. A diff over the limit is shown as its full diffstat followed by the diff cut at the limit on a
   hunk boundary, with a line naming how many bytes and files were left out.
2. A diff under the limit is shown whole, as today.
3. A test with a 20 KiB diff asserts the prompt holds the diffstat, at least one whole hunk, and
   stays within the limit plus the diffstat.
