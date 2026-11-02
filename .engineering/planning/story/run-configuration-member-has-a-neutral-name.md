---
format: aep.planning-md/3
id: story:run-configuration-member-has-a-neutral-name
kind: story
status: archived
title: A loom.agent/1 document names the run configuration member runner
summary: runner beside takt; both together refused
tags:
- loom-agent
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T10:19:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T10:19:09Z", actor: "human:timo", revision: 3}
- {from: "active", to: "archived", at: "2026-11-02T10:19:56Z", actor: "human:timo", revision: 4}
---
# Story: run-configuration-member-has-a-neutral-name

## Outcome

A `loom.agent/1` document gives takt its run configuration under a member named `runner`, as
well as under `takt`, so a downstream platform's configuration for the published image names no
product. `takt` keeps working; a document that holds both is refused naming both.

## Context

- The owner (2026-11-02): a downstream adds only configuration on top of the published image,
  and that configuration carries no product name. Every other key a deployment sets is already
  neutral: the run configuration's fields, the variables the configuration names itself
  (`endpoint_token_env`, `credential.env`), `HOME` and `XDG_STATE_HOME`.
- `crates/takt/src/run_config.rs` `from_yaml` reads only the `takt` member of a `loom.agent/1`
  document; a document with `runner` is refused today as a member that is neither Loom's nor
  `takt`.

## Acceptance

1. A `loom.agent/1` document whose `runner` member holds run configuration fields reads the same
   `RunConfig` as the same fields under `takt`, `runtime.state_dir` included as the default.
2. A document with both `takt` and `runner` is refused, the refusal naming both members and no
   value written in the file.
3. A misspelt or unknown field under `runner` is refused as under `takt`, at its line.
4. The configuration reference and the container image page name `runner` first and `takt` as
   the earlier name that still works.

## Out of Scope

- Removing the `takt` member: that would break existing documents.
- Renaming takt's own internal variables (set by takt for its own children) or its state paths.

## Scope

- `crates/takt/src/run_config.rs`, `crates/takt/tests/` (one test target),
  `website/docs/reference/configuration.md`, `website/docs/reference/container-image.md`.
