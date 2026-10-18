---
format: aep.planning-md/3
id: story:tree-hygiene
kind: story
status: implemented
title: A run never destroys or commits work it did not make
summary: Clean-tree open, exact step commits, index-aware discard, porcelain -z parsing
tags:
- wave-06
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/git.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/tree_hygiene.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T09:49:58Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-10-18T12:01:23Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
# Story: tree-hygiene

## Outcome

A run never destroys or commits work it did not make: a case refuses to open on a dirty tree, a step commits only the paths it changed, a refusal discards only what the step did (index included), and evidence is only ever bound to a revision whose content it observed.

## Context

Findings of the 2026-10-18 code review, each confirmed by an independent probe (validation table kept outside the repository):

| # | defect | where |
|---|---|---|
| 4 | no clean-tree check at open; a refusal's `checkout -- .` + `clean -fd` destroys operator work; `add -A` commits files untracked before the run | `crates/takt/src/git.rs:79`, `git.rs:98-103`, `crates/takt/src/case.rs:481-601` |
| 5 | evidence bound to HEAD while observed on uncommitted content | `case.rs:492`, `crates/takt/src/run.rs:487-491` |
| 6 | edits to already-dirty paths invisible to step guards; a harness error leaves its changes for the next commit | `run.rs:565`, `run.rs:574-579`, `run.rs:591-594` |
| 7 | `git::run` trims stdout, so the first porcelain line loses its leading space and the path its first character | `git.rs:27`, `git.rs:71` |
| 8 | quoted porcelain paths (spaces, non-ASCII) and renames not parsed | `git.rs:67-71`, `crates/takt/src/harness.rs:227-242` |
| 9 | gitignored writes not detected and not discarded | `git.rs:67`, `git.rs:101` |
| 10 | the take is an uncommitted change on the case branch; a refusal reverts the story, and the same story gets a second case | `run.rs:372`, `crates/takt/src/frontier.rs:330-341` |
| 13 | `restore` uses `remove_file` on a path the step turned into a directory; the case is destroyed | `run.rs:178-181` |
| 16 | a failed case open leaves `takt/<id>` checked out | `case.rs:564`, `case.rs:585-599` |
| 20 | `open_next` reads the store on the previous case's branch, which carries model commits | `run.rs:349`, `run.rs:362-365` |
| 28 | a `frontier.take` error leaves an orphan case and the tree on its branch | `run.rs:365-372` |
| 3 | `.git/` is not guarded: a step's write to `.git/hooks` or `.git/config` runs inside takt's own git calls | `git.rs:66-103`, `run.rs:565-578` |
| — | `discard_all`'s `checkout -- .` restores from the index, so a step that ran `git add` keeps its change through a refusal | `git.rs:100` |

## Scope

- `crates/takt/src/git.rs` (cited)
- `crates/takt/src/run.rs`: `restore` and `snapshot` (around 98-200), `open_next` (around 340-380), the judgement-step section of `Run::run` (around 540-670) (cited)
- `crates/takt/src/case.rs`: `CaseDir::open`/`create` (around 470-600) (cited)
- `crates/takt/src/harness.rs`: `is_test_path` only (around 227-242) (cited)
- new: `crates/takt/tests/tree_hygiene.rs` (inferred)

## Acceptance

On a scratch repository: opening a case on a tree with an uncommitted or untracked file is refused and leaves the tree and branch unchanged; a step that edits an already-dirty path, stages with `git add`, writes an ignored file, writes under `.git/`, or replaces a case file with a directory is refused and leaves no trace of its change; an accepted step's commit contains exactly the paths the step changed; porcelain paths with a leading space, spaces, non-ASCII characters and renames are parsed exactly; a refused or failed open leaves no case directory, no `takt/<id>` branch and the tree on its original branch; a take is committed on the case branch, so a refusal does not return the story to `proposed` and the next `open_next` does not open a second case for it; `open_next` reads the store from the base branch.

## Landed (wave 06)

Read from `git diff --stat ae94ccb impl/tree-hygiene` at the close of wave 06 (fix branch head 95995b4). The implementor's scope check found `harness.rs` `is_test_path` was not the fix site (only its test changed); the defect was the parsing in `git.rs`.

- `crates/takt/src/case.rs`
- `crates/takt/src/git.rs`
- `crates/takt/src/harness.rs`
- `crates/takt/src/run.rs`
- `crates/takt/tests/frontier.rs`
- `crates/takt/tests/gate_validator.rs`
- `crates/takt/tests/merge_under_authority.rs`
- `crates/takt/tests/tree_hygiene.rs`
- `crates/takt/tests/tree_hygiene_adv1.rs`
- `crates/takt/tests/tree_hygiene_adv2.rs`
