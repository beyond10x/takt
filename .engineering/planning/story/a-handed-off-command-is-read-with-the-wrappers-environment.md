---
format: aep.planning-md/3
id: story:a-handed-off-command-is-read-with-the-wrappers-environment
kind: story
status: implemented
title: A command handed off from a file of the tree is read with the environment and directory that file sets, and from the program word, not an option's value
summary: 'Review of 7bf9ebb F2/F3: a tree Python wrapper setting PYTHONPATH before exec let a hand commit to the imported module pass the gate; an option''s value taken as the program'
tags:
- finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T14:29:01Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-20T14:29:01Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-20T14:29:01Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a-handed-off-command-is-read-with-the-wrappers-environment

## Outcome

When the command reader hands a line off from a file of the tree to the program it is given
(`scripts/run python3 scripts/check.py`, wave 22's 3a1a39a), what that program's Python imports
is compared at the base with the environment and directory the file itself sets before it runs
the program, or the line fails closed naming the file; and the program handed off to is the
first word the file does not consume as an option's value, never an option's value such as
`--interpreter sh`.

## Context

Found by the wave 22 review at 7bf9ebb (F2, medium; F3, low), `crates/takt/src/run.rs:4849-4885`
on `wave/22`. The hand-off takes "the most the file runs is that program with the words after
it", with the caller's `dir` and `env`. Probe: `scripts/run` at the base is a Python file doing
`os.environ['PYTHONPATH']='lib'; os.execvp(argv[1], argv[1:])`, the gate runs
`sh -c 'scripts/run python3 scripts/check.py'`, and a hand commit to `lib/checklib.py` (imported
by `check.py`) let the gate run and write a `fail` record, `lib/checklib.py` never named. Not a
regression against main, which named paths only; the symlink variant is refused as a link and a
directory fails closed naming both. F3: `scripts/run --interpreter sh python3 scripts/check.py`
hands off to `sh python3 scripts/check.py`, the wrong command, so `check.py` is not read as
Python; the probe did not slip only because the wrapper's own directory mark named
`scripts/checklib.py`. Follows story:the-gate-reads-every-shell-form. Comes from
self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/run.rs` (`command_paths`: the hand-off block; a Python file of the tree read
  for `os.environ[...] =`, `os.chdir(`, `sys.path` writes and `PYTHONPATH` before `exec*`/
  `subprocess`), `crates/takt/tests/gate_compares_imports.rs`.

## Acceptance

1. The probe above, as a test: a hand commit to `lib/checklib.py` stops the gate naming it, or the
   line fails closed naming `scripts/run` and the form (`os.environ`), never a record written.
2. A Python file of the tree that changes directory (`os.chdir`) or `sys.path` before the exec
   fails the line closed naming the file and the form; one that only execs its arguments is read
   as today.
3. `scripts/run --interpreter sh python3 scripts/check.py`: the handed-off command starts at
   `python3` when `--interpreter` is read as taking a value, or the line fails closed naming the
   option the reader does not know; a test.
