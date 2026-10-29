---
format: aep.planning-md/3
id: story:the-default-confinement-is-named-as-the-default
kind: story
status: implemented
title: A run says it started with --confinement none when no flag was passed
summary: the start line names a flag the operator did not pass
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: the-default-confinement-is-named-as-the-default
relations:
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T03:40:14Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T03:40:54Z", actor: "human:timo", revision: 3, executor: "agent:takt"}
- {from: "active", to: "implemented", at: "2026-10-29T05:12:58Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# A run says it started with `--confinement none` when no flag was passed

## What is wrong

Each proof run of takt 0.18.1 (2026-10-28) printed "started with --confinement none" though the
command line passed no `--confinement`. The line reads as the operator's choice when it is the
default.

## Check

```expected_before
command: cargo test -p b10x-takt-cli --test run_console -- --exact the_default_confinement_is_named_as_the_default
exit: 101
healthy_exit: 0
output: error: no test target named `run_console`
source: crates/takt-cli/src/main.rs
```

## Outcome

A run started without `--confinement` says the confinement is the default and names it; a run
started with the flag names the flag as given.

## Acceptance

1. Without the flag, the run's start line names the confinement and says it is the default, and
   does not print `--confinement`.
2. With `--confinement none`, the line names the flag as given.

## Scope

- `crates/takt-cli/src/main.rs` (inferred)
- `crates/takt/src/console.rs` (inferred)
