---
format: aep.planning-md/3
id: story:one-run-file-for-host-and-loom-agent
kind: story
status: active
title: One run file runs takt on the host and as a LoomAgent
summary: loom 0.44.0 configFiles mount takt.yaml and llm.yaml verbatim; no takt member in loom.agent/1
tags:
- config-proxy
relations:
- decomposes: epic:run-config-proxies-llm-and-metaharness
- serves: vision:safe-agentic-coding
- depends_on: story:llm-section-is-an-llm-routes-document
- depends_on: story:model-names-an-llm-route
- depends_on: story:run-flags-spell-metaharness-and-llm
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Dockerfile
- confidence: cited
  path: crates/takt-cli/tests/run_config_cli.rs
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: examples/loom-agent
- confidence: cited
  path: website/docs/guides/deploy-on-kubernetes.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:36:42Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":6}}}
- {from: "proposed", to: "active", at: "2026-10-27T01:36:42Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":6}}}
---
# Story: one-run-file-for-host-and-loom-agent

## Outcome

One run file, with its `llm` document beside it, runs takt on the host and as a LoomAgent with the same settings: Loom
mounts both files verbatim, and the image reads them without a wrapper or a `takt` member in `loom.agent/1`.

## Context

- The owner, 2026-10-27, asked for routes and harnesses in "my config" (epic:run-config-proxies-llm-and-metaharness);
  the epic's outcome is that one file runs a host run and a LoomAgent alike.
- Loom 0.43.0 refuses an unknown `loom.agent/1` member, `takt` included (`crates/loom-sdk/src/agent.rs:177-191`).
- Loom 0.44.0 writes `spec.configFiles` verbatim at `/config/<name>` of a `Command` resource's pod and refuses a
  credential in them (`SecretLiteral`, loom 0.44.0 `CHANGELOG.md:24`). takt pins loom 0.43.0 (`Cargo.toml:16`).
- The image's entrypoint reads `/config/agent.yaml` (story:one-config-contract-with-loom); the example
  `examples/loom-agent/agent.yaml` has no `model:` member, which Loom requires (loom 0.43.0 `crates/loom-sdk/src/agent.rs:459`).

## Decision

- takt moves to loom 0.44.0. The LoomAgent example carries `spec.configFiles` with `takt.yaml` (the run file, with
  `llm: {path: llm.yaml}`) and `llm.yaml`, and `spec.command.args` is `--config /config/takt.yaml`.
- The `Dockerfile`'s `ENTRYPOINT` (`Dockerfile:54`) becomes `tini -- takt run --repo /workspace` and its `CMD`
  `--config /config/agent.yaml`, so an image started without arguments still reads `/config/agent.yaml`
  (story:one-config-contract-with-loom acceptance 5), and the example's `spec.command.args` replaces the `CMD`.
- The `loom.agent/1` reading of story:one-config-contract-with-loom stays as it is.
- Order: after story:model-names-an-llm-route (the endpoint of a mounted-files run comes from a route) and
  story:run-flags-spell-metaharness-and-llm (both extend `ess/ess-inputs.yaml`); before
  story:harness-section-is-metaharness-session-request, which moves the same loom pin again.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check`; item 4 is the end-to-end
check, run on a local kind cluster.

1. `one-file/host-and-mount-same-digest`: the run file and its llm file read from a directory and from the same bytes
   at `/config/` give `opened` records with equal `llm_digest` and equal `endpoint`.
2. `one-file/example-admitted-by-loom`: `examples/loom-agent/` is admitted by loom 0.44.0's own resource validation,
   with `model:` present and no `takt` member.
3. `one-file/literal-secret-refused-by-loom`: the example with a literal in `llm.yaml`'s `auth.secret` is refused by
   loom 0.44.0's admission with `SecretLiteral`.
4. `one-file/kind-run`: the example on a local kind cluster, its route pointed at a loopback endpoint stub in the
   cluster, writes a case whose `opened` record carries the `llm_digest` and `endpoint` scenario 1 computed on the host,
   and the stub receives at least one request for the route's upstream model.

## Out of Scope

- Loom admitting a `takt` member in `loom.agent/1`.
- Publishing the image to a registry.

## Scope

- `Cargo.toml`, `Cargo.lock` (loom 0.44.0), `examples/loom-agent/`, `Dockerfile` (entrypoint arguments),
  `website/docs/guides/deploy-on-kubernetes.md`, `crates/takt-cli/tests/run_config_cli.rs`, `ess/ess-inputs.yaml`.
