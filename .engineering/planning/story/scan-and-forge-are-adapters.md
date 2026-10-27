---
format: aep.planning-md/3
id: story:scan-and-forge-are-adapters
kind: story
status: active
title: The scan and the forge are named adapters; b10x-gates is one of them
summary: 'takt runs on any repository: scan gates|none in the run configuration, the bot forge owns every b10x-gates line'
tags:
- adapters
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/events.rs
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/forge/bot.rs
- confidence: cited
  path: crates/takt/src/forge/gitlab.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/replay.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/src/scan.rs
- confidence: inferred
  path: crates/takt/src/scan/gates.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: inferred
  path: website/docs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T00:33:20Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-27T00:33:21Z", actor: "human:timo", revision: 5}
---
## Outcome

takt runs on any repository: the scan a proposal passes and the forge a change goes through are
named adapters, and `b10x-gates` is one of them, the `gates` scan and the `bot` forge, not a tool
every run needs.

## Context

- The owner, 2026-10-27: "Why does takt documentation mention b10x-gates as some standard thing?
  this is just specific to the takt repo itself, I want to use takt for any kind of repo". His
  chosen option: scan and forge become adapters, b10x-gates one of them.
- Today a Codex or b10x-harness proposal is accepted only after `b10x-gates scan-text` scanned it
  under `B10X_GATES_POLICY` (`crates/takt/src/harness.rs`, `reads_outside_its_copy`;
  `crates/takt/src/run.rs`, `proposal_scan_refusal`: "not scanned, which is not passed"), and the
  refusal comes at the first step, not before the run. The `gitlab` forge runs the bot route's
  `b10x-gates scan-text` before every push (`crates/takt/src/forge/gitlab.rs`), and the scenario
  run's trial route builds `b10x-gates bot` lines in the CLI (`crates/takt-cli/src/main.rs`,
  `TrialRoute`).

## Design

- The scan is a seam, `crates/takt/src/scan.rs`, with two adapters: `gates` (`b10x-gates
  scan-text` under the policy `B10X_GATES_POLICY` names, `crates/takt/src/scan/gates.rs`) and
  `none` (nothing scanned). The run configuration file (`takt run --config`) names it as `scan:
  gates | none`; no flag does, so `none` is only ever an explicit declaration in a file. Not
  declared, the adapter is `gates`.
- The scan's security property holds: an unscanned proposal is never passed silently. A harness
  whose session may read outside its copy (`codex`, `harness`) under `gates` with no policy, and
  no `scan: none` declared, is refused before the case opens, naming the adapter and both ways
  out. `scan: none` declared with `B10X_GATES_POLICY` set is refused (one or the other). Under a
  declared `none` the run says so in one line before its first step and the `opened` event
  records `scan: none`; every proposal is still checked in process for the shape of a
  credential, whatever the adapter.
- The forge stays the `Forge` seam with its adapters `bot`, `gitlab` and `none`. The `bot`
  adapter moves to `crates/takt/src/forge/bot.rs` and owns every `b10x-gates bot` and `b10x-gates
  api` line, the scenario run's trial fetch and push included. `gitlab` scans what it publishes
  through the run's scan adapter: `gates` as before, nothing under a declared `none`.
- In the library and the CLI's code, the literal `b10x-gates` and `B10X_GATES_POLICY` appear only
  in `forge/bot.rs` and `scan/gates.rs`; the CLI's help text may name them where it describes
  those two adapters.
- ESS: the new noun `takt.work.ScanName` (`gates`, `none`), `scan` on
  `takt.work.RunConfiguration` and on `takt.work.RunOpened`; the forge's description names the
  scan adapter in place of the bot route's scan for `gitlab`.
- Docs: the pages that named b10x-gates as a requirement name it as the beyond10x adapter, one
  of the options (getting started, configuration, configure a run, the container image, evidence
  and the judge, run in a container).

## Acceptance

1. `takt run --harness codex` (and `--harness harness`) with no `B10X_GATES_POLICY` and no
   declaration is refused before the case opens, exit 1, the refusal naming the scan adapter
   `gates` and `scan: none` in `--config`; no case directory is written.
2. The same run with `scan: none` in its `--config` file opens its case, prints a line naming
   `scan: none` and the file before the first step, and its `opened` event holds `"scan":"none"`;
   a proposal holding the shape of a credential is still refused.
3. `scan: none` with `B10X_GATES_POLICY` set is refused before the case opens, exit 1.
4. `scan` with another value in the file is refused naming the file and the line, as any other
   bad field is.
5. Under `--forge gitlab` and a declared `scan: none`, a push runs no `b10x-gates` line; under
   `gates` it runs the same `scan-text` line as before.
6. A test over `crates/takt/src` and `crates/takt-cli/src` fails when `b10x-gates` or
   `B10X_GATES_POLICY` appears outside `forge/bot.rs` and `scan/gates.rs`, except in the CLI's
   `///` help text.
7. `task check` passes.
