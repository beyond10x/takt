---
format: aep.planning-md/3
id: story:the-command-work-source
kind: story
status: draft
title: The command work source reads items through a team's own CLI
summary: work_commands read and select print takt.work-item/1 JSON
relations:
- decomposes: epic:work-sources
- serves: vision:safe-agentic-coding
- depends_on: story:work-flag-and-the-aep-source
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/src/work/command.rs
revision: 4
---
# Story: the-command-work-source

## Outcome

The `command` source reaches a tracker no connector serves through a team's own CLI: the run file's `work_commands`
names an argv for `read` (one item by its id) and for `select` (the next ready item), each printing a
`takt.work-item/1` JSON document (`key`, `title`, `body`, `url`, `revision`, optional `labels`). `--work command:<id>`
works on one item; `"command"` in the `work` list selects the next one.

## Context

The command forge is the model (`crates/takt/src/forge/command.rs`): argv with `{placeholders}` and no shell, JSON on
standard output, `refused_exit` for a refusal, every child started through `forge::run_child` without credentials. The
item's text becomes the case's `intent.md`.

## Acceptance

- A fake `read` command printing one item: `--work command:X-1` opens a case whose `intent.md` holds its title and body
  and whose record holds its key and revision: a test.
- A `select` command printing nothing: the run says no ready work and exits as a run with no story does: a test.
- Output that is not a `takt.work-item/1` document, or names another key than asked, is refused before a case opens,
  naming the command: a test.
- The command's child has no credential in its environment beyond what the run file passes: a test.

## Scope

- new `crates/takt/src/work/command.rs` (or beside `frontier.rs`), `crates/takt/src/run_config.rs`
  (`work_commands`), `crates/takt-cli/src/main.rs`; new tests in `crates/takt-cli/tests/`.
