---
format: aep.planning-md/3
id: story:cases-live-under-dot-takt
kind: story
status: implemented
title: A run keeps its cases under .takt/cases, ignored by git by default
summary: new cases open under .takt/cases with a self-written .takt/.gitignore; legacy .engineering/cases cases resume where they are
tags:
- runtime-records
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/git.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/loop_scorecard.rs
- confidence: cited
  path: crates/takt/src/replay.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/scorecard.rs
- confidence: cited
  path: crates/takt/tests
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:44:46Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-27T01:44:46Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-27T11:33:40Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: cases-live-under-dot-takt

## Outcome

A run opens each new case under `.takt/cases/<id>/`, and when it creates `.takt/` it writes `.takt/.gitignore`
holding `*`, so `git status` on the tree shows none of the run's records and nothing of a case reaches version
control unless the repository asks for it. A case already under `.engineering/cases/<id>/` still resumes, is
approved, traced, scored and replayed where it is.

## Context

- The owner, 2026-10-27, after a trial on a downstream repository: "So currently it's stored under
  .engineering/cases. It should probably be under .takt. So all the runtime data should be probably there, and most
  likely by default we do not want this to be going into version control unless we need it."
- The trial's case was 21 files, 134,529 bytes, committed by a person in a commit of its own after takt's three.
  takt never commits a case (`crates/takt/src/git.rs:1012-1019` drops `is_under_cases` paths), but it does not
  ignore it either: status reads untracked files (`git.rs:994`) and filters them in Rust, so `git status` shows the
  case as untracked work.
- No reader of a case reads it from git: resume, approve, trace, report, scorecard, replay and the loop scorecard all
  read the working tree (`crates/takt/src/case.rs:1884-1910`, `case.rs:2724-2760`, `trace.rs:1204-1212`,
  `scorecard.rs:1079-1110`, `replay.rs:133-239`, `loop_scorecard.rs:941-966`).
- The root is one constant, `CASES_DIR` (`case.rs:37`), used in 48 places, and `is_under_cases` (`case.rs:237-242`)
  is what change detection, commit and clean leave out. The step guard refuses a proposed path under it
  (`harness.rs:373`, `harness.rs:1048`), the snapshot holds it (`run.rs:1708-1758`), `clean -e` keeps it
  (`git.rs:1268-1271`, `git.rs:2833-2835`) and link refusals guard it (`case.rs:2362-2383`).
- Hazard: checkpoint restore removes ignored paths that were not there before the step (`git.rs:2836-2841`); once
  `.takt/` is ignored, the cases root must be exempt there as well, or records written during a step are lost.
- takt's own repository commits its improvement turns under `.engineering/cases` (AGENTS.md § Layout); they stay
  there and are read as before.

## Decision

- `CASES_DIR` becomes `.takt/cases`; `LEGACY_CASES_DIR` (`.engineering/cases`) is read for a case that already
  exists there, and both roots are guarded alike. A configurable cases root is out of scope: the state directory
  (story:the-state-directory-holds-every-host-record) is the configured root of host state, and a second
  configurable root would be one more root every guard must follow.
- takt writes `.takt/.gitignore` only when `.takt/` has none, so a repository that wants its cases tracked commits
  its own and takt leaves it alone.

## Acceptance

Each item is a named test, run by `task check`.

1. `cases_under_dot_takt::a_new_case_opens_under_dot_takt`: `takt run --harness none --forge none --iterations 1
   --test-cmd true` on a clean repository writes its case under `.takt/cases/<id>/`, `.takt/.gitignore` is `*\n`,
   and `git status --porcelain` prints nothing.
2. `cases_under_dot_takt::an_existing_gitignore_is_kept`: with a committed `.takt/.gitignore` of `!cases/\n`, the
   run leaves the file byte for byte.
3. `cases_under_dot_takt::a_legacy_case_resumes_where_it_is`: a case under `.engineering/cases/<id>/` waiting on
   `repository.merge` is approved with `takt approve`, resumes and merges, and no `.takt/cases/<id>` is created.
4. `cases_under_dot_takt::a_step_writing_under_either_root_is_refused`: a harness step proposing a path under
   `.takt/cases/` or `.engineering/cases/` is refused and its change put back, as today for the legacy root.
5. `cases_under_dot_takt::records_written_during_a_step_survive_its_undo`: a step that is refused and undone leaves
   the case's `events.jsonl` and `evidence/` as the run wrote them, with `.takt/` ignored.
6. `cases_under_dot_takt::readers_find_both_roots`: `takt trace`, `takt scorecard` and the loop scorecard list a
   case under each root.

## Scope

- `crates/takt/src/case.rs`: `CASES_DIR`, `LEGACY_CASES_DIR`, `is_under_cases`, case open and resume (cited)
- `crates/takt/src/git.rs`: commit filters, `clean -e`, checkpoint restore of ignored contents (cited)
- `crates/takt/src/run.rs`, `crates/takt/src/harness.rs`, `crates/takt/src/replay.rs`,
  `crates/takt/src/loop_scorecard.rs`, `crates/takt/src/scorecard.rs`, `crates/takt-cli/src/main.rs` (cited)
- tests naming `.engineering/cases` under `crates/*/tests` (69 files, cited by grep)

## Out of scope

- Moving takt's own recorded improvement turns.
- A configurable cases root.
