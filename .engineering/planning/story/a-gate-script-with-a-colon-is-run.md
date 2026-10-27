---
format: aep.planning-md/3
id: story:a-gate-script-with-a-colon-is-run
kind: story
status: active
title: A gate script holding the shell's colon builtin is run, not a pathspec error
tags:
- finding
relations:
- decomposes: epic:composable-agentic-work
- informed_by: review-result:adversary-u2-pass-1
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T02:32:10Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-27T02:32:11Z", actor: "human:timo", revision: 3}
---
## Outcome

A gate whose script contains the shell's `:` builtin is run and judged like any other gate,
instead of `gate.run` ending in a git pathspec error.

## Context

- review-result:adversary-u2-pass-1, F4: a gate script holding `:` makes takt read
  `git ls-tree … -- :/__main__.py`, which git refuses ("Unimplemented pathspec magic '_'"), in
  `object_at` (`crates/takt/src/validator.rs`), reached from the import reading of a gate's
  command (`python_paths`, `python_imports`). The function is unchanged by wave 38; takt's own
  gates enter through `scripts/gate-exec` and do not reach it.
- Reproduction: `cargo test -p b10x-takt --test failed_gate_lines_adv1
  a_gate_script_with_the_colon_builtin` passes while the defect is present; the case names this
  story.

## Acceptance

1. That case asserts the gate is run and fails or passes on its command's own exit.
2. A path that starts with `:` is passed to git as a literal path (`:(literal)` or
   `--literal-pathspecs`).
