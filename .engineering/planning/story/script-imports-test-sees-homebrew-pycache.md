---
format: aep.planning-md/3
id: story:script-imports-test-sees-homebrew-pycache
kind: story
status: draft
title: The script-imports outside-path test sees Homebrew Python write its own cache
tags:
- finding
- macos
refs:
- provider: finding
  reference: script-imports-test-sees-homebrew-pycache
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# The script-imports outside-path test sees Homebrew Python write its own cache

## What is wrong

`a_program_written_to_a_directory_on_path_outside_the_tree_is_refused`
(`crates/takt/tests/step_guard_sees_test_script_imports.rs:1171`) failed on macos-15 in the
darwin job at e10d0a6 (run 39614054772) and passed at 6ec2c58 (run 39604433453). Its `concurrent`
variant refused, besides the `newtool` it expects, three paths under
`/opt/homebrew/Cellar/python@3.14/3.14.7/Frameworks/Python.framework/Versions/3.14/lib/python3.14/venv/__pycache__`
(`__init__.cpython-314.pyc`, `__main__.cpython-314.pyc`) as written by `tests.run`.

Inferred, not traced: Homebrew's Python standard library is writable by the runner's user, so
the first `import venv` on the host compiles its bytecode into the Cellar. When that import runs
during the variant's step (another test in the same binary runs in parallel, or the step's own
Python), the guard sees a write to an import directory outside the tree and refuses it. On Linux
the standard library is root-owned and nothing is written. Wave 28's unit C (4339d92) changed only
the `usersite` variant of this test, not `concurrent`.

## Change

Decide the cause first, by running the variant on macos-15 alone and with the binary's other
tests. Then make the test's Python write no bytecode outside its tree (`PYTHONDONTWRITEBYTECODE`
or `PYTHONPYCACHEPREFIX` in the variant's environment), or compile the standard library before the
step. The guard is unchanged: a write by the step to an import directory outside the tree is still
refused.

## Check

Not runnable by `scripts/check-finding` (Linux under bubblewrap, where nothing is written). The
machine that checks it is the darwin job on macos-15. It reproduced in 1 of the 2 darwin runs on
record, so one green run does not show the defect gone.

```expected_before
host: macos-15, .github/workflows/darwin.yml job test-library
command: cargo test -p b10x-takt --test step_guard_sees_test_script_imports a_program_written_to_a_directory_on_path_outside_the_tree_is_refused
exit: 101
healthy_exit: 0
output: venv/__pycache__
source: .github/workflows/darwin.yml
```
