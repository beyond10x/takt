---
format: aep.planning-md/3
id: story:the-state-directory-holds-every-host-record
kind: story
status: implemented
title: The state directory holds every host record and defaults to the Loom agent document's runtime.state_dir
summary: set-aside joins transcripts under --state-dir; loom.agent/1 runtime.state_dir is the default
tags:
- loom-agent
- runtime-records
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
- depends_on: story:run-state-directory-is-configurable
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:44:46Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-27T01:44:47Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-27T11:33:40Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
# Story: the-state-directory-holds-every-host-record

## Outcome

The state directory a run is given (`--state-dir`, the run configuration's `state_dir`,
story:run-state-directory-is-configurable) holds every record the run keeps on the host, its transcripts and its
set-aside files alike, and a run whose configuration is a `loom.agent/1` document takes that document's
`runtime.state_dir` as the state directory when neither the flag nor `takt.state_dir` names one.

## Context

- The owner, 2026-10-27: "maybe what we do want, I could imagine some of that store, some of the things is being
  created through Loom and Canon. So that probably needs some base tier configuration or something like that, where
  we tell it where to store the stuff."
- story:run-state-directory-is-configurable puts transcripts under the state directory it is given. The set-aside
  directory, where discarding a step moves the files it rewrote inside an ignored directory, is still
  `$XDG_STATE_HOME/takt/set-aside` (`crates/takt/src/run.rs:7919-7930`), so a pod whose `HOME` is not kept loses
  them as it lost transcripts.
- Loom's agent document already carries the base-tier key: `runtime.state_dir` (loom `ess/domains/agent.yaml:354-358`,
  `loom.agent.RuntimeSpec`). takt reads only the document's `takt` member (`crates/takt/src/run_config.rs:17-31`).
- Canon prescribes no location (it takes files as arguments); the decisions and observations of Loom's governor are
  written through takt's case store into the case directory, so they need no root of their own.
- Upstream follow-up, Loom's, not this story: `loom-agent serve` reads only `runtime.connectors` of that member and
  takes its own state root from `--state` (loom `crates/loom-agent/src/serve.rs:129-134`,
  `crates/loom-cli/src/lib.rs:533-534`). Falling back to `runtime.state_dir` there would put Loom's own records
  under the same root.

## Acceptance

Each item is a named test, run by `task check`.

1. `state_dir_holds_host_records::set_aside_goes_under_the_state_dir`: a run given `--state-dir <dir>` whose
   discarded step rewrote a file inside an ignored directory moves it under `<dir>/set-aside/`, and nothing is
   written under `$XDG_STATE_HOME/takt`.
2. `state_dir_holds_host_records::loom_runtime_state_dir_is_the_default`: a `loom.agent/1` configuration with
   `runtime: {state_dir: <dir>}` and no `takt.state_dir` keeps the run's transcripts under `<dir>/transcripts/`.
3. `state_dir_holds_host_records::takt_state_dir_wins_over_runtime`: the same document with `takt.state_dir: <b>`
   keeps them under `<b>`; `--state-dir <c>` wins over both.
4. `state_dir_holds_host_records::no_state_dir_is_todays_location`: without any of the three, transcripts and
   set-aside are where they are today.

## Scope

- `crates/takt/src/run_config.rs`: reading `runtime.state_dir` of a `loom.agent/1` document (cited)
- `crates/takt/src/run.rs`: set-aside under the state directory (cited)
- `crates/takt-cli/src/main.rs`: precedence of flag, `takt.state_dir` and `runtime.state_dir` (cited)

## Depends on

story:run-state-directory-is-configurable (the `--state-dir` flag and key).

## Measured

Measured on the trial of 2026-10-27 (one case, two runs): the state directory held 5
transcripts of 820,205 bytes for the case, from 32,234 to 511,991 bytes each, and no set-aside or replay files. The
case directory in the tree was 134,529 bytes. Host state is therefore most of what a run writes, and losing it with
an unkept `HOME` loses the larger part of the record.
