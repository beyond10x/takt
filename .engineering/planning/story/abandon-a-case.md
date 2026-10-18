---
format: aep.planning-md/3
id: story:abandon-a-case
kind: story
status: draft
title: takt abandon ends a case that cannot finish
summary: takt abandon records explicitly_abandoned, returns the story to proposed, deletes the branch; a closed PR stops the run
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/abandon_a_case.rs
revision: 4
---
# Story: abandon-a-case

## Outcome

An operator can abandon a case that cannot finish, and a case whose pull request was closed stops instead of spending its budget. After `takt abandon <case>` the next argument-less `takt run` takes the next story.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4).

- Every argument-less `takt run` resumes the first unfinished case before it takes a story (`crates/takt/src/run.rs:477-491`), and a story a case was opened for is never taken again (`run.rs:516-522`). A case that cannot finish blocks the frontier for good.
- Two ways to get there: the pull request was closed on GitHub, which `pr.open` records as the state it reads, `pull_request closed` (`crates/takt/src/validator.rs:756-781`), after which model steps edit until the budget ends; or the story itself is wrong.
- The protocol's `abandoned` outcome requires the decision `explicitly_abandoned` (`protocols/story-delivery/1.yaml:155-158`); nothing in takt supplies that decision.

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- `grep -rn explicitly_abandoned crates/` prints nothing.
- `takt abandon --help` exits 2 (no such subcommand).

## Scope

- `crates/takt-cli/src/main.rs`: the `takt abandon <case>` verb (inferred)
- `crates/takt/src/case.rs`: recording the `explicitly_abandoned` decision (inferred)
- `crates/takt/src/run.rs`: resume and take skipping an abandoned case (cited)
- `crates/takt/src/validator.rs`: a closed pull request as a stop (cited)
- `crates/takt/tests/`: an abandon test (inferred)

## Acceptance

`takt abandon <case>` records `explicitly_abandoned`, after which the case's outcome is `abandoned`, its story is back at `proposed`, its branch is deleted and the next argument-less `takt run` takes the next ready story; and a run whose `pr.open` reads the pull request `closed` stops at once with a stop that names the closed pull request.
