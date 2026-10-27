---
format: aep.planning-md/3
id: epic:protocol-settings-in-the-run-file
kind: epic
status: draft
title: Every protocol's declared settings are set in the repository-local run file
summary: protocol key, protocols section, shipped defaults, a listing
relations:
- serves: vision:safe-agentic-coding
- informed_by: epic:run-config-proxies-llm-and-metaharness
revision: 1
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
  (`main.rs` `RunArgs.inputs`, `:594`). It names the protocol only with `--protocol` (`main.rs:573`). The run file has
  neither (`run_config.rs:71-91`).
- The first use: `story.delivery/2` (engineering-protocols `story:story-delivery-2-with-settings`) declares
  `tests_first` and `gate_on`.

## Stories

- `story:the-run-file-sets-protocol-settings`
- `story:takt-ships-default-protocol-settings`
- `story:takt-lists-a-protocols-settings`

## Out of scope

- New protocols. This epic only sets what protocols declare.
- A second configuration file.
