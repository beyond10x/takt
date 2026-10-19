---
format: aep.planning-md/3
id: story:git-filter-drivers-do-not-run
kind: story
status: draft
title: No git call takt makes runs a filter driver the repository names
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
revision: 1
---
## Outcome

Every git invocation takt makes through `git::command`, and the checkout in `takt promote`'s clone, runs with every filter driver a `.gitattributes` could name disabled (`filter.<name>.smudge=`, `clean=`, `process=`, `required=false` for each configured name, or an equivalent), so no repository chooses a command takt's git runs.

## Context

From the review of main at fe93286 on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding P1.4 (code confirmed; no filter in this machine's global config), and the developer's report of 2026-10-19: the override exists only on `crates/takt-cli/build.rs`'s own git calls; `git.rs:22-24` says "A filter driver a `.gitattributes` names is not covered"; `promotion.rs:1292@fe93286` checks out through `git()` (`:532-552`), which blocks hooks and fsmonitor only. Planner decision: a story of its own rather than an item of story:wave11-review-follow-ups.

## Scope

- `crates/takt/src/git.rs`: `command` (cited)
- `crates/takt/src/promotion.rs`: `git()` (cited)

## Acceptance

With a global git config defining `filter.mark.smudge` as a script that writes a marker, a repository whose `.gitattributes` names `filter=mark`, checked out by `takt run`'s checkpoint restore and by `takt promote`'s clone, writes no marker.
