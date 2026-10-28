# Wave 41: the run console, headless

Stage: **open**. Shape: one dispatcher in one managed worktree (`takt-wave-41`), one wave branch
`wave/41` cut from `main` at 338082d, one shared target directory, builds one at a time.

The wave merges after wave 40. Wave 40 also changes `crates/takt-cli/src/main.rs` and
`crates/takt/src/case.rs`, so wave 41 takes `main` with wave 40 in it before its own merge.

The owner asked on 2026-10-28, watching a self-run of takt on one of its own stories, for
`takt run` output a developer can follow: the story first, less frontier noise, and what the
harness does as it does it. This wave delivers the headless part that needs no Loom change. The
live session lines (story:run-console-shows-the-session-live) wait on
dependency-blocker:loom-session-event-observer; a terminal UI (story:run-console-tui) is a later
milestone.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:run-console-opens-with-the-story | `crates/takt/src/console.rs` (new), `run.rs`, `case.rs`, `crates/takt-cli/src/main.rs`, `crates/takt-cli/tests/run_console_cli.rs` | |
| U2 | story:run-console-steps-are-compact | `crates/takt/src/run.rs`, `console.rs`, `crates/takt-cli/src/main.rs`, `crates/takt-cli/tests/run_console_steps.rs` | |

U2 starts after U1 is committed: both change the same printing code.

## Log

- U1 dispatched.
