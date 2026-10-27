---
format: aep.planning-md/3
id: story:exit-4-reads-the-same-in-run-and-scenario-help
kind: story
status: draft
title: takt run --help and takt scenario run --help say the same about exit 4
summary: one resume command for ApprovalRequired, or the difference stated
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: exit-4-reads-the-same-in-run-and-scenario-help

## Outcome

`takt run --help` and `takt scenario run --help` say the same thing about exit 4, or say why they
differ.

## Context

- `crates/takt-cli/src/main.rs:958` (`takt run`): "4  stopped: ApprovalRequired; `takt approve`
  grants it and the next run resumes the case".
- `crates/takt-cli/src/main.rs:971` (`takt scenario run`): "4  stopped: ApprovalRequired; `takt
  approve` grants it and `takt run --case <case>` resumes the case".
- The site's generated exit-code page prints both, so a reader sees two answers to one question
  (found while writing the site's guides, 2026-10-26).

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check`.

1. `help/exit-4-names-one-resume`: the exit-4 lines of both helps name the same resume command, or
   the scenario line says why a scenario's case is resumed by `--case` (its case is not the open
   case of a story the frontier takes); a test compares the two lines.

## Scope

Cited: `crates/takt-cli/src/main.rs` (`EXIT_CODES`, `SCENARIO_EXIT_CODES`); inferred:
`crates/takt-cli/tests/`.
