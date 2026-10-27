---
format: aep.planning-md/3
id: story:python-modules-are-kept-only-when-a-python-answers
kind: story
status: active
title: A case keeps python-modules.json only when a Python answered
summary: no interpreter, no file; the guard uses the built-in lists as before
tags:
- runtime-records
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: story:step-guard-sees-test-script-imports
scope:
- confidence: cited
  path: crates/takt/src/replay.rs
- confidence: cited
  path: crates/takt/src/run.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:44:47Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-27T01:44:48Z", actor: "human:timo", revision: 4}
---
# Story: python-modules-are-kept-only-when-a-python-answers

## Outcome

A case keeps `python-modules.json` only when a Python interpreter answered at case open. When none answered, the
case keeps no such file and the step guard uses the built-in lists exactly as it does when the file holds them: at
open, at resume and in a replay.

## Context

- The owner, 2026-10-27, on a trial case: "why do I now there also have a file called Python modules? I'm not using
  Python and I don't want to."
- `Modules::of_case` (`crates/takt/src/run.rs:2850-2858`) writes the file at every case open (`run.rs:8769`) and
  resume (`run.rs:9039`), whatever the test command runs. On the trial, whose test command runs no Python, it held
  4,945 bytes of the built-in lists, with `from` "no Python interpreter answered".
- The names guard the import directories a test command's Python loads from (story:step-guard-sees-test-script-imports;
  `run.rs:2518-2523`). With no interpreter, the guard's input is the built-in lists (`Modules::default`), which takt
  can rebuild at any time: the file adds nothing a later run could not compute.
- `takt replay` copies the file when the case has it (`replay.rs:48-58`), and `Modules::read` falls back to the
  built-in lists when it is absent (`run.rs:2861-2868`).

## Acceptance

Each item is a named test, run by `task check`.

1. `python_modules_file::no_python_no_file`: a case whose test command is `true`, with no Python on `PATH`, opens
   with no `python-modules.json`, and a step that writes a stdlib-named module into an import directory the built-in
   lists cover is refused as before.
2. `python_modules_file::a_python_writes_the_file`: with a Python on `PATH` named by the test command, the file is
   written as today.
3. `python_modules_file::resume_without_the_file_writes_none`: resuming the case of item 1 writes no file and guards
   the same names.
4. `python_modules_file::replay_without_the_file`: `takt replay` of a step of the case of item 1 rebuilds its tree
   without the file and judges the step as the run did.

## Scope

- `crates/takt/src/run.rs`: `Modules::of_case` and its callers (cited)
- `crates/takt/src/replay.rs`: the opening files (cited)
