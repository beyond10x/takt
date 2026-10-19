---
format: aep.planning-md/3
id: story:takt-lists-a-trees-cases
kind: story
status: draft
title: takt lists a tree's cases with their state
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

`takt cases --repo <tree>` lists every case under `.engineering/cases/` with its id, protocol, outcome or open state, the approval it waits for if any, its last step and the command that resumes it.

## Context

From the planner's operator-path variety round on takt at 12cffde (2026-10-19), scenario 1 at seed 865eaa8 with no origin and a fake harness. `takt cases` and `takt status` exit 2 ("unrecognized subcommand"); the only listing is `takt scorecard`, which shows no case's open or pending state. An operator who left a case waiting on an approval has to read `events.jsonl` by hand to find it.

## Scope

- `crates/takt-cli/src/main.rs`: a `cases` subcommand (inferred)
- `crates/takt/src/case.rs`: reading a case's state (inferred)

## Acceptance

On a tree holding one accepted case, one stopped NoProgress and one waiting on `repository.merge`, `takt cases` prints three rows with those states, and the waiting row names `takt approve` and the resume command.
