---
format: aep.planning-md/3
id: story:scenario-corpus
kind: story
status: draft
title: Each scenario run starts from a fresh trial base with no hand reset
summary: trial/<n> per run on the scratch repository; PR targets it; story proposed again
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: scenario-corpus

## Outcome

A scenario run starts from a known state without any hand reset: each run gets a fresh base branch
`trial/<n>` on the scratch repository, cut from the scenario's seed commit, the case's pull request
targets that branch, and the scratch store on it shows the scenario's story `proposed`.

## Context

design:recursive-self-improvement § Scenario corpus. The two live trials of 2026-10-18 each needed a
hand reset (case directory moved, branch renamed, a remote branch deleted, the store file
restored). The forge carries a base for the pull request (`crates/takt/src/forge.rs:79`). A
scenario is declared in the scratch repository, not in takt: its story id, its seed commit and its
`expected_before` predicates.

## Domain relations

- Scenario → Story: one scenario names exactly one story of the scratch repository's store.
  UNMAPPED: whether a scenario may later name several stories.
- Scenario run → Case: one run opens one case. Inferred from `crates/takt/src/run.rs`, one run one case.

## Scope

- `crates/takt-cli/src/main.rs` (a `--base` for the case's pull request, or a `scenario run` verb) (inferred)
- `crates/takt/src/forge.rs` (the base passed through) (inferred)
- the scratch repository: a `scenarios/` file listing scenario id, story id, seed commit (inferred)

## Ordering

- Before anything: verify that the bot route admits creating `trial/<n>` on the scratch repository and opening a pull request against it; if it does not, the story stops with that refusal.
- story:scorecard-reader follows this story on `crates/takt-cli/src/main.rs`.

## Acceptance

Two scenario 1 runs in a row, with no command between them other than the second run, each start
with `story:remove-a-todo` `proposed` and each open a pull request whose base is its own `trial/<n>`.

## Checks

- `gh pr view <n> --json baseRefName` names `trial/<n>` for both runs.
- `git status` in the scratch checkout between the runs shows no change made by hand.

## Out of Scope

The scorecard (story:scorecard-reader); several scenarios per run.

## Ambiguities

- Where a scenario is declared: `inferable`, in the scratch repository, because the scenario is
  that repository's state; takt only reads it.
