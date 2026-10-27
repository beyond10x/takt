---
format: aep.planning-md/3
id: story:the-run-file-sets-protocol-settings
kind: story
status: active
title: The run file names the protocol and sets its declared inputs
summary: protocol key and protocols section, checked against case.inputs
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:protocol-settings-in-the-run-file
- depends_on: story:one-run-file-for-host-and-loom-agent
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/tests/protocol_settings.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T12:00:46Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-27T12:00:46Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
---
# Story: the-run-file-sets-protocol-settings

## Outcome

The run file gains two things:
- **A `protocol` key.** It names the protocol a store story opens on. Today that is the built-in default
  (`crates/takt-cli/src/main.rs:2715`, 110251a) or `--protocol`.
- **A `protocols` section** that sets declared inputs per protocol:
  `protocols: {story.delivery: {tests_first: required, gate_on: after_handover}}`.

Each value is checked against the protocol's declared `case.inputs` values (`protocol.rs:95-121`). An unknown protocol,
input or value is refused at its line, as `RunConfig` refuses an unknown key (`run_config.rs:71`,
`deny_unknown_fields`).

Precedence, highest first:
1. `--input`
2. a story tag `input:<name>=<value>`
3. the run file
4. takt's built-in defaults (`story:takt-ships-default-protocol-settings`)

A story tag `protocol:<name/rev>` chooses the protocol for that story. Every value is recorded with its source on
`opened` and in `case.yaml`.

## Context

- Operator, 2026-10-27: protocols declare their settings, and the repository-local takt configuration sets them.
- The section sits in the same file as `llm:` and `harness:`, so `story:one-run-file-for-host-and-loom-agent` carries
  it to a LoomAgent with no second surface.

## Acceptance

Each is a named conformance scenario run by `task check`.

`software.change/1` declares `risk: [trivial, standard, elevated, critical]` (engineering-protocols
`protocols/software-change/1.yaml:23`).

1. `protocol_settings::the_run_file_sets_an_input`: a run file setting `software.change` `risk: elevated` opens a
   case whose `case.yaml` records `risk: elevated` with source `run-file`.
2. `protocol_settings::an_unknown_value_is_refused_at_its_line`: `risk: extreme` is refused with the file, line and
   the four declared values.
3. `protocol_settings::a_tag_overrides_the_run_file`: with the run file at `elevated`, a story tagged
   `input:risk=trivial` opens with `trivial`, source `tag`.
4. `protocol_settings::the_flag_overrides_the_tag`: `--input risk=critical` over that tag opens with `critical`,
   source `flag`.
5. `protocol_settings::the_protocol_key_chooses_the_protocol`: `protocol: software.change/1` opens a store story on it.
6. `protocol_settings::a_protocol_tag_overrides_the_key`: with `protocol: software.change/1` in the run file, a story
   tagged `protocol:story.delivery/1` opens on `story.delivery/1`, and `opened` records source `tag`.

## Scope

- `crates/takt/src/run_config.rs` (`protocol`, `protocols`)
- `crates/takt/src/protocol.rs` (input resolution with source)
- `crates/takt-cli/src/main.rs` (precedence with `--input`, `--protocol`)
- `crates/takt/src/case.rs` (`case.yaml` inputs with source)
- `ess/domains/takt.yaml` (the setting source, before code)
- `crates/takt/tests/protocol_settings.rs`
