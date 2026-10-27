---
format: aep.planning-md/3
id: story:a-takt-prefixed-base-is-named-as-reserved
kind: story
status: draft
title: A base branch under takt's reserved prefix is refused by name
summary: the refusal names takt/ and a rename
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
scope:
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests
revision: 2
---
# Story: a-takt-prefixed-base-is-named-as-reserved

## Outcome

A base branch whose name starts with takt's reserved prefix `takt/`, and that is not a case branch, is refused before a
case opens. The refusal is exit 1 and names the prefix and the fix: "branch `takt/x` uses takt's reserved prefix
`takt/`; base branches cannot start with it; rename it (e.g. `git branch -m takt/x trial/x`)".

A case branch is `takt/<id>` with an existing case directory, and it keeps resolving to its recorded base as today.

## Context

- A downstream trial (2026-10-27) started from a base named under `takt/`. It was refused as "a case's branch with no
  base recorded", a message about a case that did not exist.
- Cause: `on_a_case_branch`/`to_base` (`crates/takt/src/run.rs:8053-8085` at 110251a) treat every branch with
  `BRANCH_PREFIX = "takt/"` (`case.rs:40`) as a case branch.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `reserved_prefix::a_takt_prefixed_base_is_named`: in a repository on branch `takt/x` with no case `x`, `takt run`
   exits 1 before any case directory is created, and stderr contains `reserved prefix` and `git branch -m`.
2. `reserved_prefix::a_case_branch_still_resolves`: on `takt/<id>` with that case open, the run resumes on the case's
   recorded base as before.

## Scope

- `crates/takt/src/run.rs` (`on_a_case_branch`, `to_base`)
- `crates/takt/src/case.rs` (`BRANCH_PREFIX`)
- `crates/takt/tests/` (conformance scenarios)
