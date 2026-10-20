---
format: aep.planning-md/3
id: story:a-hand-off-is-read-from-an-allow-list
kind: story
status: draft
title: The reader takes a hand-off only from an allow-listed form, and a tree directory on PATH is compared at the base
summary: 'Review of wave 23 (c3685a3) B-1..B-6: 11 probes slip the deny-list of hand-off forms or plant a program in a tree PATH directory and the gate records pass'
tags:
- finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-hand-off-is-read-from-an-allow-list

## Outcome

The command reader takes a hand-off from a file of the tree to a program it knows only when the
file's whole text is one the reader can prove runs that program with the words after it, in the
line's environment and directory (an allow-list of forms), and fails the line closed otherwise;
and a tree directory a wrapper or a shell form puts on `PATH` is compared at the base like any
other import directory, so a program planted there cannot run under a gate that recorded `pass`.

## Context

Found by the review of wave 23 at c3685a3 (B-1 to B-6, file for later). Wave 23's
story:a-handed-off-command-is-read-with-the-wrappers-environment (092ab01) fails the hand-off
closed on a deny-list of text forms (`hand_off_unread`, `python_hand_off_form`,
`crates/takt/src/run.rs:2569`, `2612` on `wave/23`). The review's probes passed the gate with:

- B-1, B-3 (medium): a Python wrapper that puts a tree directory on `PATH` and `execvp`s a bare
  program, and four shell forms doing the same; a planted `bin/python3` ran and the gate recorded
  `pass`. The gate never compares a tree directory a wrapper puts on `PATH`
  (`crates/takt/src/validator.rs:1001`, from before wave 23). Reachable only when the base wrapper
  already puts a tree directory on `PATH`.
- B-4 (medium): `python3 scripts/run python3 scripts/check.py` with a wrapper that sets
  `PYTHONPATH`: the hand-off block runs only for a program in none of the tables, so an
  interpreter running the wrapper is not checked.
- B-2 (low-medium): an alias, `e = os.environ; e['PYTH'+'ONPATH']=…`, slips the text match
  (`run.rs:2691`), though the doc says a name that hides it is caught.
- B-5, B-6 (low): a tree wrapper given a tree script; a wrapper importing a helper that sets
  `PYTHONPATH`; `env PYTHONPATH=…` passed through `execvp`'s arguments.

A deny-list of forms cannot be complete. Comes from self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/run.rs` (the hand-off reader, `command_paths`'s interpreter branch for a tree
  script that hands off), `crates/takt/src/validator.rs` (`PATH` directories of the tree compared
  at the base), `crates/takt/tests/gate_compares_imports.rs`.

## Acceptance

1. Each of the review's 11 slipping probes (B-1 to B-6) is a test in `gate_compares_imports.rs`;
   each stops the gate naming the path or fails the line closed naming the file and the form;
   none writes a record.
2. The hand-off is read only from a file whose text matches an allow-listed form (for Python:
   imports of `os` and `sys` only, one `os.execv*`/`os.execvp*` of `sys.argv[1:]`, nothing else
   that runs); every other file fails the line closed naming it. takt's own `scripts/gate-exec`
   is in that form or named as a reviewed exception, and `takt_gates_document` stays green.
3. A tree directory on the line's `PATH`, from the binding's `env`, an `export`, a wrapper, or a
   shell form, is compared at the base, and a program planted there stops the gate naming it.
