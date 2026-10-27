---
format: aep.planning-md/3
id: epic:run-config-proxies-llm-and-metaharness
kind: epic
status: draft
title: 'takt''s run file proxies llm and metaharness: model routes and harnesses in their own words'
summary: llm and harness sections that are llm.routes/1 and metaharness's session request, validated by their own code; same flag names
tags:
- config-proxy
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# Epic: run-config-proxies-llm-and-metaharness

## Outcome

An operator configures takt's model routes and its harnesses in the words llm and metaharness already use: the run
file carries an `llm` section that is an `llm.routes/1` document and a `harness` section that is metaharness's session
request, each parsed and validated by that library's own code, and takt adds only the keys it owns. The command line
spells the same keys as `metaharness run` and llm do, and one file runs a host run and a LoomAgent alike. Every run
records the digest of the llm document it ran under on its `opened` record, so that a host run and a LoomAgent run can
be shown to have read the same routes.

The owner, 2026-10-27, verbatim: "what I'm missing for takt is basically, under the hood, I think it's using the llm
library. It's also using the metaharness library. So shouldn't we then sort of proxy the configuration of those?
Meaning, for example, I would be able to configure model routes in my config, or I'm able to just configure Claude,
Codex, and so on in the same sort of way how I would do that if I'm running metaharness."

## Context

From the design note `takt-config-proxy.md` (2026-10-27), versions read: takt 0.14.0 on loom 0.43.0, which pins
llm 0.8.1 and metaharness 0.19.0; newest llm 0.16.0, metaharness 0.19.0, loom 0.44.0.

- takt builds no llm client: `--harness harness` is a metaharness session through Loom's session executor
  (`crates/takt/src/harness.rs:527-529`), metaharness spawns `b10x-harness run`, and that child builds one client from
  one target. takt's endpoint settings are its own re-typed copy: the endpoint file (`crates/takt/src/harness.rs:2603-2671`),
  the `--endpoint-*` flags (`crates/takt-cli/src/main.rs:1604-1653`), two wires (`harness.rs:2806`), the default model
  `claude-opus-5` (`harness.rs:2715`).
- takt hard-codes max turns 40, a step timeout of 1800 s and the credential source (`harness.rs:265-267`); Loom's
  executor hard-codes the rest of metaharness's `RunSpec` per kind (loom 0.43.0
  `crates/loom-session-executor/src/lib.rs:791-836`). Effort, the vendor cost cap and the version pin check are never
  set.
- llm has a validated document, `llm.routes/1`, parsed by `Routes::parse` (YAML) and `Routes::parse_toml`, with
  unknown keys refused and errors that name a field and never a value (llm 0.16.0
  `crates/llm-models/src/routes.rs:107`, `:141-148`, `:1484`, `:1510`, `:63-72`).
- metaharness has no document: `RunSpec` is spelt as `metaharness run <kind>` flags and builder calls (metaharness 0.19.0
  `crates/metaharness-protocol/src/spec.rs:165`), with no `deny_unknown_fields`; `check_spec` validates it
  (`crates/metaharness/src/builder.rs:1860-1976`).
- `loom.agent/1` refuses an unknown member such as `takt` (loom 0.43.0 `crates/loom-sdk/src/agent.rs:177-191`); Loom
  0.44.0 writes `spec.configFiles` verbatim at `/config/<name>` (loom 0.44.0 `CHANGELOG.md:24`).

## Decision (proposed, for the owner)

Option A of the design note: passthrough sections validated by their libraries, with a by-path form for `llm` only
(`llm: {path: <file>}`) because Loom's `model.routes` already names an llm file by path. Rejected: B, takt re-declares a
subset (a third copy of every key, drifting each release); C, separate files by path for both (several files per run,
and metaharness has no file form). No ambient lookup: takt reads an llm document only where the run names one.

## Stories

story:llm-section-is-an-llm-routes-document, story:model-names-an-llm-route,
story:run-flags-spell-metaharness-and-llm, story:one-run-file-for-host-and-loom-agent,
story:harness-section-is-metaharness-session-request, story:harness-flags-spell-metaharness-names.

## Out of Scope

- metaharness publishing its session request as a document (`RunSpec` with unknown keys refused, a public parse and
  `check_spec` over it): metaharness's story. story:harness-section-is-metaharness-session-request waits on it.
- Loom's session executor taking a caller's partial `RunSpec` over its per-kind defaults: Loom's story.
- Fallback across routes, retries and HTTP timeouts as configuration: llm's `llm.routes/1` has none of them (llm 0.16.0
  `crates/llm-models/src/routes.rs:1436-1445`; `crates/llm-routing/src/fallback.rs:35-54`).
- Removing the `--endpoint-*` flags and the endpoint file: a later story after the alias release.
