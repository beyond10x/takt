---
format: aep.planning-md/3
id: story:test-pass-needs-the-runners-report
kind: story
status: draft
title: A test pass needs the runner's own completion report, not only exit 0
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:step-guard-sees-test-script-imports
revision: 1
---
## Outcome

A test run cannot be recorded as `pass` because the code under test ended the test process early with exit 0: `tests.run` records `pass` only when the runner's own completion report agrees with the exit status (for unittest the final `OK` line and its count, for cargo the `test result:` lines), and records `fail` with the reason when the exit is 0 but the report is missing or short.

## Context

From the developer's check of story:step-guard-sees-test-script-imports (turn 4, 2026-10-18): with `unittest.py`, `sitecustomize.py` and `usercustomize.py` guarded, a step can still make the test command exit 0 while a test fails: code under test that calls `os._exit(0)` on import ends the interpreter before unittest reports (python 3.14). No path guard can close it, since the code under test is the change itself. `tests.run` takes its result from the exit status alone (`crates/takt/src/validator.rs`, `result` from `output.success()`); story:test-summary-keeps-the-totals (merged at 4cd2dd4) made the summary text faithful but does not change the result.

## Scope

- `crates/takt/src/validator.rs`: `TestsRun::observe`, the result (cited)
- `crates/takt/tests/`: a step whose code calls `os._exit(0)` on import (inferred)

## Acceptance

On scenario 1 with a fake harness whose edit adds `import os; os._exit(0)` to `todo.py` beside a failing test, `tests.run` records `fail` naming the missing completion report, and the case does not reach the merge approval; an ordinary passing run still records `pass`.
