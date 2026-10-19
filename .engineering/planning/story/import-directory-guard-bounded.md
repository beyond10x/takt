---
format: aep.planning-md/3
id: story:import-directory-guard-bounded
kind: story
status: draft
title: The import-directory guard is bounded and blames only the step
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: story:step-guard-sees-test-script-imports
revision: 1
---
## Outcome

The guard over the directories a test script imports from (story:step-guard-sees-test-script-imports, turn 4) costs a bounded amount per run, and it stops a run only for a change the step made to a file the test runner loads.

## Context

From the grant review of turn 4 at 1e2fdb3 (2026-10-19, approved). Each holds at 1e2fdb3:

- W1 `crates/takt/src/run.rs:4970` (the walk under `outside_import_dirs`, `:1476`): the walk has no cap. With `PYTHONPATH=..` over 1,000,000 files it took 218 s and 1.76 GB per run. It should fail closed past a fixed entry count, naming the directory.
- W2: another process writing under `PYTHONPATH=..` while a step runs stops the run, and the stop blames `tests.run`.
- W3 `run.rs` `Modules::ask` (about 1736 and 1802): an in-tree virtual environment locks every Python file. Read `pyvenv.cfg`'s `home` and ask the base interpreter instead.
- W4 `run.rs:1535-1550,1863`: the caller's user site is snapshotted, though the test runs with `PYTHONNOUSERSITE=1`.

## Scope

- `crates/takt/src/run.rs`: `outside_import_dirs`, the snapshot walk, `Modules::ask` (inferred)
- `crates/takt/tests/`: the import-guard tests of turn 4 (inferred)

## Acceptance

- A test script with `PYTHONPATH` naming a directory over the cap stops the step before the walk finishes, naming the directory and the cap.
- A file written under an import directory by a process that is not the step does not stop the run.
- A repository with an in-tree `.venv` runs a step that edits one non-imported Python file without a stop.
- The user site is not snapshotted when the test runs with `PYTHONNOUSERSITE=1`.
