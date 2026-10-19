---
format: aep.planning-md/3
id: story:merge-checks-the-pull-request-base
kind: story
status: draft
title: The merge refuses a pull request whose base is not the case's base
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
revision: 1
---
## Outcome

Before merging, `repository.merge` reads the pull request's base branch and refuses when it differs from the case's `base`, naming both.

## Context

From the review of main at fe93286 on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding C4.5, confirmed: the pull request view has no base field (`forge.rs:253-258@fe93286`, `view_argv` at `:963`), `validator.rs:960-969` repairs only the branch name, and `base` in `branch.yaml` is read unchecked. A pull request retargeted on the forge merges into the other branch and is recorded merged. story:judge-guard D4 checks `base_commit` ancestry at the merge, not the pull request's base.

## Scope

- `crates/takt/src/forge.rs`: the view's fields (cited)
- `crates/takt/src/validator.rs`: `PrMerged` (cited)

## Acceptance

With a fake forge whose pull request reports `baseRefName: other`, the merge observation records no merge and names `other` and the case's base; with the matching base it merges as today.
