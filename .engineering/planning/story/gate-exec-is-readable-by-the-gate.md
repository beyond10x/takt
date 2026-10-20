---
format: aep.planning-md/3
id: story:gate-exec-is-readable-by-the-gate
kind: story
status: implemented
title: takt's own gate commands are readable by the gate's command reader
summary: scripts/gate-exec's if/case lines make every takt gate fail closed on any Python or shell change
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:01:41Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-20T04:01:41Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-20T05:16:23Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: gate-exec-is-readable-by-the-gate

## Outcome

takt's own gate commands can be read by the command reader the step guard and `gate.run` share,
so a delivery on takt that changes a Python or shell file the gate does not run is judged by its
gate instead of stopping every gate by failing closed.

## Context

Since story:gate-compares-python-run-through-a-shell (wave 19), a gate line the reader cannot
read with certainty fails closed: every Python file, shell file and program is compared at the
base. takt's gates run through `scripts/gate-exec`, whose `if` and `case` lines the reader cannot
follow, so any takt delivery that changes a `.py`, `.sh`, `.bash` or `.pth` file or a program
stops all of takt's gates. Rust-only deliveries are unaffected. Comes from self-improvement work,
so it carries `rsi`.

## Scope

- `scripts/gate-exec`, `.engineering/gates.yaml`, `.engineering/assertions-project.yaml`
- `crates/takt-cli/tests/takt_gates_document.rs`

## Acceptance

1. `takt`'s gate document binds commands the reader reads with certainty: no gate line of takt's
   fails closed (a test reads the summary of a gate run on a delivery that changes an unrelated
   `.py` file and finds no fail-closed reason).
2. A change to a script a gate does run still stops that gate, naming it: a test.
3. The gate still runs the same five commands `task check` runs.
