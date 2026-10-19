---
format: aep.planning-md/3
id: story:check-finding-builds-name-their-revision
kind: story
status: implemented
title: A takt built by check-finding names its revision
tags:
- finding
refs:
- provider: finding
  reference: check-finding-builds-name-their-revision
relations:
- informed_by: review-result:findings-hunt-3fe3f21
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T23:06:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T23:06:59Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-18T23:07:07Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A takt built by `scripts/check-finding` prints its revision in `takt --version`, and records it as `binary` on `opened`.

## Context

`check-finding` exports the tree with `git archive` (`scripts/check-finding:163`). With no `.git` there, `build.rs` reports `unknown revision`. So every promoted-versus-candidate verification run records `"binary":"unknown revision"`, and nothing ties its figures to a revision. Observed: my archive build of 8378cb2 prints `takt 0.1.0 (unknown revision)`. The existing cached builds `~/.cache/takt/check-finding/builds/98719c3…` and `d74da0c…` print the same. A build from a checkout prints the sha. This predicate only reproduces under `check-finding`'s own build, which is where the defect is.

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
