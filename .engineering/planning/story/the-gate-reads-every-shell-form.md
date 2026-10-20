---
format: aep.planning-md/3
id: story:the-gate-reads-every-shell-form
kind: story
status: draft
title: The gate reads or fails closed on every shell form that runs Python
summary: five of 34 probed shell forms and several wrapper programs still let a change decide the gate
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
revision: 4
---
# Story: the-gate-reads-every-shell-form

## Outcome

Every shell form a gate's command can use to run Python is either read, so the script and what it
imports are compared at the base, or failed closed, so no change to them decides the gate without
the gate naming it.

## Context

Wave 19's review probed 34 shell forms against `gate.run`. Story
gate-compares-python-run-through-a-shell and its fix round close 26; 3 run the code under work at
the tree's root by wave 18's decision. These still let a hand commit to a module the gate's Python
imports decide the gate, each an ignored test in `crates/takt/tests/gate_compares_imports.rs`
naming this story:

- `pypath`: `PYTHONPATH=lib python3 scripts/check.py`
- `exportpp`: `export PYTHONPATH=lib; python3 scripts/check.py`
- `source`: a sourced file sets `PYTHONPATH` by assignment and `export`
- `dashm`: `cd scripts && python3 -m check`, a module beside the one `-m` runs
- `xargs`: `echo scripts/check.py | xargs python3`
- `exec 0< FILE; sh`, `exec < FILE; sh`, `exec < FILE; bash`: a redirection-only `exec` sets the
  shell's standard input for the commands after it, and the reader drops it (found by the wave 19
  final confirm)
- `env -C scripts python3 -m check`: as `dashm`
- `cd scripts && python3 -c "import checklib…"` and `cd scripts && python3 - < check.py`: after a
  `cd`, the directory these forms import from is not compared (the root rule of wave 18 covers only
  the tree's root); found by the wave 19 confirm

The same review found, beyond the 34: wrapper programs the reader does not know run Python without
failing closed (`taskset`, `chrt`, `unbuffer`, `script -qec`, `uv run`, `env -S`, `pytest`), and
`env PYTHONPATH=lib` and `env -C dir python3 -m x` slip as their shell forms do. Comes from
reviewing self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/run.rs` (the command reader the step guard and the gate share)
- `crates/takt/src/validator.rs` (the gate's Python comparison)
- `crates/takt/tests/gate_compares_imports.rs`, `crates/takt/tests/step_guard_sees_test_script_imports.rs`

## Acceptance

1. The eleven ignored tests in `crates/takt/tests/gate_compares_imports.rs` marked for this story
   pass without `#[ignore]`.
2. A `PYTHONPATH` entry set in the line, by `export`, in a sourced file or by `env` is taken as the
   binding's own `PYTHONPATH` is: its `-m` modules and `*.pth` files compared.
3. `cd` and `env -C` change the directory the reader takes a command to run in.
4. A program the reader does not know that takes another program as an argument (the wrappers
   above, `xargs`) is read when it is listed, else the line fails closed: a test per wrapper.
5. The step guard sees what it saw before for every readable line; each place it becomes stricter
   is named in the story's close.
