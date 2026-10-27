---
format: aep.planning-md/3
id: story:run-flags-spell-metaharness-and-llm
kind: story
status: draft
title: takt's endpoint and llm flags spell metaharness's and llm's names
summary: --model-endpoint, --model-wire, --subscription-token-file/-pointer, --api-key-secret, --llm-config; --endpoint-* stay as aliases
tags:
- config-proxy
relations:
- decomposes: epic:run-config-proxies-llm-and-metaharness
- serves: vision:safe-agentic-coding
- depends_on: story:model-names-an-llm-route
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 6
---
# Story: run-flags-spell-metaharness-and-llm

## Outcome

`takt run` and `takt scenario run` take the endpoint and llm settings under the names `metaharness run` and llm use,
so a flag learnt on one works on the other; the `--endpoint-*` flags keep working for one minor release as aliases.

## Context

- The owner, 2026-10-27: "in the same sort of way how I would do that if I'm running metaharness"
  (epic:run-config-proxies-llm-and-metaharness).
- takt's endpoint flags: `--endpoint-base-url`, `--endpoint-wire`, `--endpoint-model`, `--endpoint-token-file`,
  `--endpoint-token-pointer`, `--endpoint-secret`, `--endpoint-token-env`, `--endpoint-file`
  (`crates/takt-cli/src/main.rs:1604-1653`), shared by both commands through `LoopArgs` (`:688-782`).
- metaharness's: `--model-endpoint`, `--model-wire`, `--subscription-token-file`, `--subscription-token-env`,
  `--subscription-token-pointer`, `--api-key-secret` (metaharness 0.19.0 `crates/metaharness-protocol/src/spec.rs:313-355`).
  llm's: `--config` and `--model` (llm 0.16.0 `crates/llm-cli/src/lib.rs:55`, `:88-99`); takt's `--config` is its own
  run file, and `--model` is already shared.

## Decision

- Five new flags with metaharness's names and meanings: `--model-endpoint`, `--model-wire`, `--subscription-token-file`,
  `--subscription-token-pointer`, `--api-key-secret`; and `--llm-config <file>` for the llm
  document (llm's `--config`), which wins over the run file's `llm` section.
- Alias table, each old flag printing one stderr line naming its new name: `--endpoint-base-url` → `--model-endpoint`,
  `--endpoint-wire` → `--model-wire`, `--endpoint-token-file` → `--subscription-token-file`,
  `--endpoint-token-pointer` → `--subscription-token-pointer`,
  `--endpoint-secret` → `--api-key-secret`. `--endpoint-model` is an alias of `--model`. `--endpoint-token-env` and
  `--endpoint-file` stay as they are: takt stages that variable as an API key (story:endpoint-credential-from-environment),
  which is not metaharness's `--subscription-token-env`, and the endpoint file stays
  until it is removed. Removal of the aliases is a later story.
- `--model-wire` takes takt's two wires under their current names.
- The new flags and their aliases are endpoint sources: beside an `llm` section they are refused as
  story:model-names-an-llm-route decides (one endpoint source per run).
- Order: after story:model-names-an-llm-route, because both change `harness_of` in `crates/takt-cli/src/main.rs` and
  `ess/ess-inputs.yaml`.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check` with `--harness harness`
against a loopback endpoint.

1. `flags/new-names-set-endpoint`: for each of the five endpoint flags, a run setting it alone gives the same `opened`
   record `endpoint` as the equivalent endpoint-file key, on `takt run` and on `takt scenario run` (one case per flag).
2. `flags/aliases-warn-once`: for each of the six aliases, a run setting it gives the same `opened` record as its new
   name and prints exactly one stderr line naming the new name (one case per alias).
3. `flags/llm-config-over-section`: `--llm-config b.toml` over a run file's `llm` section gives `b.toml`'s
   `llm_digest` in the `opened` record.
4. `flags/help-lists-new-names`: `takt run --help` and the generated CLI reference list all six new flags.

## Out of Scope

- Flags for the harness section's keys: story:harness-flags-spell-metaharness-names.
- Removing the aliases and the endpoint file.

## Scope

- `crates/takt-cli/src/main.rs` (`LoopArgs`, `RunArgs`, `parsed`), `crates/takt-cli/tests/`,
  `website/docs/reference/configuration.md`, `ess/ess-inputs.yaml`.
