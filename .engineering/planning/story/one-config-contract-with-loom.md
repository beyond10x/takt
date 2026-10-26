---
format: aep.planning-md/3
id: story:one-config-contract-with-loom
kind: story
status: active
title: 'One configuration contract with Loom: --config reads /config/agent.yaml'
summary: --config by extension, TOML or YAML; a loom.agent/1 document's takt member is the run configuration
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: cited
  path: Dockerfile
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/run_config_cli.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: ess/domains/takt.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 9}
---
# Story: one-config-contract-with-loom

## Outcome

takt's image starts on the file Loom mounts for a `Command` resource, `/config/agent.yaml`, with
no wrapper: one configuration contract between the two.

## Context

- takt 0.12.0's image reads `--config /config/run.toml` (TOML); Loom 0.42.0 writes `spec.agent`
  verbatim to `/config/agent.yaml` and mounts nothing else there (`docs/design/loom-agent-crd-deployment.md`
  § 9a). The trial overrode the entrypoint with a shell wrapper and passed every setting as a flag.
- Loom's admission decodes `spec.agent` as `loom.agent/1` and refuses an unknown member
  (loom 0.42.0 `crates/loom-sdk/src/agent.rs`, `unknown member`), so a `takt` member is refused
  by a loom release that does not admit it.

## Decision

Recorded in approval-record:wave-37-takt-as-a-loom-agent, the orchestrator's recommendation taken:

- `--config <FILE>` reads TOML when the file ends in `.toml` and YAML when it ends in `.yaml` or
  `.yml`; another extension is refused. Both forms take the same fields (`takt.work.RunConfiguration`).
- A YAML file whose `format` is `loom.agent/1` is a Loom agent document: its `takt` member, when
  present, is the run configuration, with the same fields; its other members are Loom's and are
  not read by takt; without a `takt` member it sets nothing, and the resource's `spec.command.args`
  carry the run's flags.
- The image's entrypoint reads `/config/agent.yaml`.
- Until Loom admits a `takt` member in `loom.agent/1`, a resource carries the run's settings as
  flags in `spec.command.args`; that admission is loom's gap.

## Acceptance

1. `--config run.yaml` (plain YAML) and `--config run.toml` set the same run from the same fields;
   an unknown key in either is refused before any case opens, exit 1, naming the file and the line
   and repeating no value.
2. `--config agent.yaml` with `format: loom.agent/1` and a `takt` member sets the run from that
   member; an unknown key inside it is refused as in 1; Loom's members are not read.
3. A `loom.agent/1` document with no `takt` member sets nothing, and the run takes its flags.
4. A file of another extension is refused before any case opens, exit 1, naming the file.
5. The `Dockerfile` entrypoint is `takt run --repo /workspace --config /config/agent.yaml`.

## Out of Scope

- Loom admitting a `takt` member.

## Scope

- `crates/takt/src/run_config.rs`, `crates/takt-cli/src/main.rs` (`RunArgs::config` help),
  `crates/takt-cli/tests/run_config_cli.rs`, `ess/domains/takt.yaml` (`takt.work.RunConfiguration`),
  `Dockerfile` (entrypoint), `README.md` § Container.
