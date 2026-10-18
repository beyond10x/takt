---
format: aep.planning-md/3
id: story:scenario-corpus
kind: story
status: implemented
title: Each scenario run starts from a fresh trial base with no hand reset
summary: trial/<n> per run on the scratch repository; PR targets it; story proposed again
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/scenario_run.rs
- confidence: inferred
  path: crates/takt/src/git.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T12:07:49Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-18T12:07:50Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-18T13:04:29Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":3,"verification":1}}}
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

Derived 2026-10-18 by `story-scoper` against branch `fix/2026-10-18-review` at 768db1c (before tree-hygiene and validator-confinement merged). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt-cli`: a `scenario run` verb that cuts `trial/<n>` from the seed commit, pushes it through the bot route, checks it out, then runs the normal path; or a `--base` on `run` — inferred, the story's own scope marks it inferred
- **Files:** `crates/takt-cli/src/main.rs:28-37` (`Commands`), `:54-117` (`RunArgs`), `:189-420` (the `run` dispatch) — inferred
- **Already in place:** the case's base is the branch the tree is on when the case opens, `crates/takt/src/case.rs:837-859`; it reaches the pull request through `crates/takt/src/validator.rs:398-416` and `crates/takt/src/forge.rs:79,418`. A run started on `trial/<n>` already opens its pull request against `trial/<n>` — cited
- **Not touched:** `crates/takt/src/forge.rs`. The story's "base passed through" already exists, and `Forge::push` (`forge.rs:406`, `push_argv` `:273-277`) takes any branch name, so pushing `trial/<n>` reuses it unchanged — inferred
- **Also likely:** `crates/takt/src/git.rs:43` (`create_branch` takes a `from`; `<n>` needs a fetch or ls-remote helper) — inferred
- **Also likely:** `crates/takt-cli/tests/scenario_run.rs`, new — inferred
- **Outside this repository:** the scenario declaration (scenario id, story id, seed commit, `expected_before`) sits in the scratch repository (seed `865eaa8`, per decision-blocker:live-trial-repository). The location is cited from the story; the file name `scenarios/…` is inferred
- **Documents:** none in takt — inferred
- **Confidence:** medium. The base mechanism is read from the tree. Whether the entry point is a takt verb, a `--base` (that would move the change into `case.rs:837-856`, not `forge.rs`) or a script in the scratch repository is the story's own inferred choice. In the script case takt changes nothing
- **Would collide with:** any unit touching `crates/takt-cli/src/main.rs` `Commands`, `RunArgs` or the run dispatch. story:scorecard-reader follows on this file (cited from the story's Ordering) — inferred for the rest
- **Safety fact:** a run that stops before its first commit can leave the store file dirty; `git checkout trial/<n+1>` keeps it, and run 2 would read `active` and resume run 1's case. The setup must refuse a dirty tree, not discard it. (Read against 768db1c; story:tree-hygiene, merged after, commits the take and refuses a dirty open.) — level 3, unproven
- **Bot-route precheck (Ordering), not run:** `b10x-gates bot` checks only the git verb and hook bypass, no branch-name rule; `api` has no path allow-list. Still to check: (a) a GitHub ruleset or branch protection on `trial/*` in the scratch repository (read-only `gh api …/rulesets`, `…/branches`); (b) the App installation's `contents:write` and `pull_requests:write`; (c) a pre-push hook in the scratch checkout (bot pushes do not bypass hooks; a destination that is not enrolled is refused); (d) the live proof: `b10x-gates bot -- push origin trial/0`, then `b10x-gates api --method POST --path /repos/<owner>/<repo>/pulls` with base `trial/0`. (a) and (d) are network calls, left to the story's first step — cited for the gates lines, inferred for (a) to (c)

Typed entries (all inferred): `crates/takt-cli/src/main.rs`, `crates/takt/src/git.rs`, `crates/takt-cli/tests/scenario_run.rs`.

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

## Landed (wave 07)

Read from `git diff --stat c1445fa 85f2a75` at the close of wave 07 (wave branch head ea832ee). The entry point is a `takt scenario run` verb; the declaration is `.engineering/scenarios.conf` in git config syntax; the bot-route fetch and lease push are built in `main.rs` (TrialRoute), not `forge.rs` (follow-up).

- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/scenario_run.rs`
- `crates/takt-cli/tests/scenario_run_adv1.rs`
- `crates/takt-cli/tests/scenario_run_adv2.rs`
- `crates/takt/src/git.rs`
