---
format: aep.planning-md/3
id: story:scan-before-push
kind: story
status: draft
title: pr.open scans commit messages and the diff before it pushes
summary: scan-text over base..head messages and diff before push_revision; a refused scan leaves no branch on the remote
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/scan_before_push.rs
revision: 4
---
# Story: scan-before-push

## Outcome

`pr.open` scans everything it is about to publish before anything leaves the machine: the commit messages of `base..head` and the diff, as well as the pull request's title and body. A refused scan leaves no branch on the remote.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4).

- `pr.open` pushes the case's branch first (`crates/takt/src/validator.rs:751-753`, `push_revision`) and only then opens the pull request, where scan-text runs over the request and the published title and body (`crates/takt/src/forge.rs:875-882`).
- The pushed commits carry model-written content and free-text subjects: a step's commit subject is `<action>: <summary>`, the summary the harness reported (`crates/takt/src/run.rs:1066`). None of it is scanned.
- A refused scan therefore leaves the branch public; the refusal says so (`forge.rs:878`), but the content is out.

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- A test with a stub forge and a case whose step commit subject contains a literal the policy forbids: `pr.open` calls `push_revision` before any scan-text call, and no scan-text call's input contains the commit messages or the diff of `base..head`.

## Scope

- `crates/takt/src/validator.rs`: `PrOpen::observe`, a scan before `push_revision` (cited)
- `crates/takt/src/forge.rs`: a scan argv over `git log --format=%B base..head` and the diff (cited)
- `crates/takt/tests/`: a stub-forge call-order test (inferred)

## Acceptance

With a stub forge, `pr.open` runs scan-text over `git log --format=%B base..head` and the diff of `base..head` before `push_revision`, and when that scan refuses, `push_revision` is never called and the observation records nothing.
