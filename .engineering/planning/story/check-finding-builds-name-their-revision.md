---
format: aep.planning-md/3
id: story:check-finding-builds-name-their-revision
kind: story
status: draft
title: A takt built by check-finding names its revision
tags:
- finding
refs:
- provider: finding
  reference: check-finding-builds-name-their-revision
relations:
- informed_by: review-result:findings-hunt-3fe3f21
revision: 1
---
## Outcome

A takt built by `scripts/check-finding` prints its revision in `takt --version`, and records it as `binary` on `opened`.

## Context

`check-finding` exports the tree with `git archive` (`scripts/check-finding:163`). With no `.git` there, `build.rs` reports `unknown revision`. So every promoted-versus-candidate verification run records `"binary":"unknown revision"`, and nothing ties its figures to a revision. Observed: my archive build of 3fe3f21 prints `takt 0.1.0 (unknown revision)`. The existing cached builds `~/.cache/takt/check-finding/builds/834205f…` and `8d49743…` print the same. A build from a checkout prints the sha. This predicate only reproduces under `check-finding`'s own build, which is where the defect is.

```expected_before
scenario: 1
command: "$TAKT" --version
exit: 0
output: (unknown revision)
source: .engineering/planning/review-result/findings-hunt-3fe3f21.md
```

## Scope

`scripts/check-finding` (the build step). Possibly `crates/takt-cli/build.rs`, if the fix is a revision the caller passes in.

## Acceptance

`check-finding` at revision R builds a binary whose `--version` prints R (or `R-dirty`, never `unknown revision`), and the `opened` line records R.
