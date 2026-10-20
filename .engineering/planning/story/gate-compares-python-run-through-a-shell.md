---
format: aep.planning-md/3
id: story:gate-compares-python-run-through-a-shell
kind: story
status: implemented
title: gate.run compares a Python script run through a shell, and what it imports
summary: a gate running Python through sh -c or a shell script compares neither the script nor its imports
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T01:48:33Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T01:48:33Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T03:02:45Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: gate-compares-python-run-through-a-shell

## Outcome

`gate.run` compares at the base a Python script a gate runs through a shell, and what that script
imports, as it compares one the gate runs directly: `sh -c "python3 scripts/check.py"`, and a
shell script whose lines run Python, no longer let a step's change to the script or its modules
decide the gate.

## Context

Found by wave 18's adversarial review (2026-10-20): `runs_python` is read from a binding's
top-level words only (`crates/takt/src/validator.rs`, `python_paths`), so a Python run inside
`sh -c` or a shell script is not seen. Probes `shc`, `shscript`, `shcself` and `shscriptself` each
let a hand commit pass the gate. For the script itself the gap predates wave 18: at 3913af6a a
hand commit to a script run through `sh -c` already passed. Comes from reviewing
self-improvement work, so it carries `rsi`. The step guard reads test commands the same way
(`crate::run::scripts_of`); a fix shared with it is preferred over a second reader.

## Scope

- `crates/takt/src/validator.rs`
- `crates/takt/src/run.rs` (if `scripts_of` learns to read `sh -c` strings and shell scripts)
- `crates/takt/tests/gate_compares_imports.rs`

## Acceptance

1. A gate whose command is `sh -c "python3 scripts/check.py"` is not run after a step changes
   `scripts/check.py` or a module beside it, naming the file: a test.
2. The same for a shell script the gate runs whose lines run Python: a test.
3. A shell line takt cannot read with certainty in the forms this story closes fails closed,
   naming why: a glob or brace word where the reader names what runs, an `exec` or `command`
   option it does not know, an input a shell reads as its script other than a tree file or
   `/dev/null`. Amended at the close (wave 19, by the dispatcher on the review): the story
   closes 26 of the review's 34 probed shell forms (`sh -c`, shell scripts, `exec` with its
   options, `sh < FILE`, `cat FILE | sh`, glob and brace words among them); 3 run the code under
   work at the root by wave 18's decision; the 5 still slipping are ignored tests and
   story:the-gate-reads-every-shell-form. Fix round 2 reads a descriptor before a redirection as
   part of it (`sh < run.sh 2>&1`, `sh 0< run.sh`, `python3 2>/dev/null check.py`); of 22 further
   forms the confirm probed, 20 are closed and 2 (`cd dir && python3 -c …`,
   `cd dir && python3 - < x.py`) are in that story. A shell's standard input is read only when
   the same command redirects or pipes it from a tree file: `exec 0< FILE; sh` and its two
   siblings, and `env -C dir python3 -m x`, slip and are ignored tests in that story (the final
   confirm; corrected by the owner's delegate's ruling, A).

## Predicate

To be drafted with the next batched predicate run, from the review's probes.
