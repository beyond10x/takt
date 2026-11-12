---
format: aep.planning-md/3
id: story:a-run-on-a-schedule
kind: story
status: draft
title: takt run stays resident and starts one pass per slot of a schedule in a named time zone
summary: --every, --window, --time-zone, --until, the run file's schedule, tick files and --outcome-file
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/run_schedule_cli.rs
- confidence: cited
  path: crates/takt/Cargo.toml
- confidence: cited
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: crates/takt/src/schedule.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 13
---
# Story: a-run-on-a-schedule

## Outcome

`takt run --every <duration>` stays resident and starts one pass per slot: inside each `--window` (days and a wall-clock
start, an optional end) in `--time-zone`, the start and every `every` after it up to its end; no window, a slot every
`every` from the start. Each pass is a child `takt run` with the same arguments, the schedule flags removed and
`--once` and `--outcome-file <tick dir>/outcome.json` added. One tick runs at a time: a tick still running at its next
slot skips that slot (counted, never queued), and after a suspend the missed slots run once. It stops when an `--until`
condition holds (`nothing-ready`, `exit:<n>`, `outcome:<name>`, `deadline:<instant>`) and writes every tick to files.
SIGTERM or SIGINT starts no further tick, lets the running tick end, writes `end.json` with reason `signal` and exits
3; nothing is forwarded to the tick (`design:continuous-orchestration` § 5, a pause read at a boundary); a second
signal is forwarded. A second start of the same schedule while `schedule.json` names a live pid is refused, exit 1,
naming the pid; after a dead pid or an `end.json`, it starts again and runs no slot `ticks.jsonl` already holds. The
run file's `schedule` mapping (`every`, `time_zone`, `windows`, `until`) sets the same, a flag winning. Without
`--every` and without `schedule` nothing changes.

## Context

Design § 3 (`docs/design/scheduled-runs-and-connector-actions.md`). `takt run` is one pass today (`Commands::Run`,
`crates/takt-cli/src/main.rs:1857`); "nothing ready" is only a stdout line (main.rs:2473-2475), so `--outcome-file`
(`takt.run-outcome/1`: `exit`, `stop`, `case`, `outcome`, `frontier`, `started`, `ended`) comes with this story.
Zones through `jiff` with its bundled tz database (`jiff` 0.2.38 is in `Cargo.lock`). No new exit code
(`design:continuous-orchestration` § 5).

Shared file in wave 57: `crates/takt/src/lib.rs`, one `pub mod schedule;` line; story:one-served-connectors-client adds
one `pub mod` line there too. This unit is committed first; the other's line is added after it.

## Acceptance

- `run_schedule_slots_berlin`: windows `Mon-Tue 07:00-22:30` and `Wed 07:30`, every 30m, Europe/Berlin: 32 slots on a
  Monday, 32 on Tuesday, 1 on Wednesday, 0 on Thursday (unit test over fixed instants).
- `run_schedule_slots_dst`: a slot in the spring-forward gap moves to the first valid instant; a slot in the autumn
  fold is taken once, at the earlier instant (unit test).
- `run_schedule_slots_without_window`: no window, every 10m from 12:00: the next three slots are 12:10, 12:20, 12:30
  (unit test).
- `run_schedule_missed_slots_run_once`: the next slot after an instant five slots late is one slot, the first one not
  in the past, and the skip count is 5 (unit test).
- `schedule_until_nothing_ready`: `--every 1s --until nothing-ready` with a `command` source whose `select` prints
  nothing: `ticks.jsonl` has 1 line with exit 0, `ticks/<slot>/outcome.json` has `frontier: nothing_ready`, `end.json`
  has reason `nothing-ready`, exit 0.
- `schedule_until_exit` and `schedule_until_deadline`: each ends with exit 0 and its reason in `end.json`.
- `schedule_until_outcome`: a tick whose case reaches outcome `X` ends `--until outcome:X` with exit 0.
- `schedule_refused_tick_ends_it`: a tick exiting 2 ends the schedule with exit 2.
- `schedule_busy_tick_skips_a_slot`: a tick that runs longer than `every`: the next `ticks.jsonl` line has `skipped`
  of at least 1 and no two ticks overlap in their started/ended times.
- `schedule_sigterm_ends_after_the_tick`: SIGTERM during a tick: that tick's line has its own exit, `end.json` has
  reason `signal`, exit 3.
- `schedule_second_signal_forwarded`: a second SIGTERM during the same tick reaches the tick: its line has a
  non-zero exit, `end.json` reason `signal`, exit 3.
- `schedule_second_start_refused`: a second start while the first runs: exit 1 naming the first's pid.
- `schedule_run_file_key`: the run file's `schedule` alone starts the same schedule as the flags (same `schedule.json`
  `resolved` member).
- `schedule_flag_wins`: `--every 2s` over a file's `every = "1s"` gives `every` of 2s in `schedule.json`.
- `schedule_run_file_refusals`: an unknown member, a zone that is no IANA name, an `every` below 1s, a window that does
  not parse: each refused at its line, exit 2.
- `outcome_file_without_schedule`: `--outcome-file` on a run that opens a case and on one that finds nothing ready:
  both files are `takt.run-outcome/1` with the right `case` and `frontier`.
- Spec first: `takt.work.Schedule`, `takt.work.ScheduleTick`, `takt.work.RunOutcome` and the field `schedule` on
  `takt.work.RunConfiguration` in `ess/domains/takt.yaml`; `ess specify validate --path ess` exits 0.

## Scope

- new `crates/takt/src/schedule.rs`; `crates/takt/src/lib.rs` (one line); `crates/takt/src/run_config.rs` (`schedule`);
  `crates/takt-cli/src/main.rs` (flags, `--once`, `--outcome-file`, the resident loop); `crates/takt/Cargo.toml`,
  `Cargo.toml` and `Cargo.lock` (`jiff`); `ess/domains/takt.yaml`; new `crates/takt-cli/tests/run_schedule_cli.rs`;
  `website/docs/reference/configuration.md` (the field table and the flags).
