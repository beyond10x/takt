---
format: aep.planning-md/3
id: epic:protocol-settings-in-the-run-file
kind: epic
status: implemented
title: Every protocol's declared settings are set in the repository-local run file
summary: protocol key, protocols section, shipped defaults, a listing
relations:
- serves: vision:safe-agentic-coding
- informed_by: epic:run-config-proxies-llm-and-metaharness
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T19:04:02Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:planner"}
- {from: "proposed", to: "active", at: "2026-10-28T19:04:02Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:planner"}
- {from: "active", to: "implemented", at: "2026-10-28T19:04:02Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:planner"}
---
# Epic: protocol-settings-in-the-run-file

## Outcome

Every protocol takt runs declares its settings, variants and options as Canon `case.inputs`. The repository-local
takt run file names the protocol and sets those inputs. takt ships its defaults as data and can list them. There is
one configuration surface: the same run file that holds `llm:` and `harness:` (`epic:run-config-proxies-llm-and-metaharness`,
`story:one-run-file-for-host-and-loom-agent`), so a LoomAgent reads the same settings.

## Context

- Operator, 2026-10-27: every protocol declares its settings, variants and options, and the repository-local takt
  configuration sets them.
- Canon case inputs are already a protocol's settings, "fixed when the case opens" (canon `crates/canon/src/model/case.rs:41-44`).
  Canon has no default for an input.
- takt parses inputs (`crates/takt/src/protocol.rs:23-24`, `:95-121`, 110251a) but sets them only with `--input`
  (`crates/takt-cli/src/main.rs` `RunArgs.inputs`, `:594`). It names the protocol only with `--protocol` (`main.rs:573`). The run file has
  neither (`run_config.rs:71-91`).
- The first use: `story.delivery/2` (engineering-protocols `story:story-delivery-2-with-settings`) declares
  `tests_first` and `gate_on`.

## Stories

- `story:the-run-file-sets-protocol-settings`
- `story:takt-ships-default-protocol-settings`
- `story:takt-lists-a-protocols-settings`

## Acceptance

The epic is done when, on a fixture repository:
- a run file holding `protocols: {story.delivery: {gate_on: each_revision}}` opens a store story on
  `story.delivery/2` with `tests_first: required` (source `default`) and `gate_on: each_revision` (source
  `run-file`);
- `takt protocol show story.delivery/2` prints both inputs with those values and sources.

## Out of scope

- New protocols, and new inputs on the other built-ins. This epic only sets what protocols declare. Of the eight
  engineering-protocols built-ins, `software.change/1` already declares inputs and `story.delivery/2` gets them in
  engineering-protocols `story:story-delivery-2-with-settings`. Declaring the variants of any other protocol is that
  protocol's own story in engineering-protocols, and the code of this epic needs no change for it.
- A second configuration file.
