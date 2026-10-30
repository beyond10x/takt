---
format: aep.planning-md/3
id: story:git-and-model-through-loopback-relays
kind: story
status: implemented
title: Push, fetch and model calls go through loopback relays with no credential in takt
summary: http to a loopback origin and a keyless loopback model endpoint
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/takt-cli/tests/loopback_relays_cli.rs
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T03:35:01Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-30T03:35:01Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-30T18:09:08Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":7}}}
---
# Story: git-and-model-through-loopback-relays

## Outcome

A run whose `origin` is `http://127.0.0.1:<p>/<owner>/<repo>.git` fetches and pushes through it,
and a run whose model endpoint is `http://` to a loopback host runs with no credential: takt
holds neither the forge's token nor the model's key.

## Context

In a pod with no secret in its container, a boundary container on loopback relays git
smart-HTTP to a Connectors instance (`LOOM_BOUNDARY_GIT_URL`, `http://127.0.0.1:8403`), which
checks the ref policy, the commit identity and the pre-push scan and pushes with the App's
installation token; and relays model calls to the LLM gateway (`LOOM_BOUNDARY_MODEL_URL`,
`http://127.0.0.1:8401/v1`).

Today takt pins git to `GIT_ALLOW_PROTOCOL=https` (`crates/takt-cli/src/main.rs`, `FETCH_ENV`,
and the forges' push environment), and a model endpoint needs a credential source
(`crates/takt/src/harness.rs:4571`); an llm route of `auth: {kind: anonymous}` is not accepted.
`http://` is already admitted for a loopback host only (`harness.rs:5047`, `loopback` at
`:5085`).

The pre-push scan runs on the Connectors instance in this mode, so the run's own scan of what it
publishes is `none`, and says so in the case.

Loom 0.51.0 serves `spec.boundary`: relays on 127.0.0.1:8401 (model), 8402 (connectors) and 8403 (git), plain `http://` only. The takt side is built and tested against loopback stand-ins; the run through the real git relay waits for the Connectors release that serves it.

## Scope

- `crates/takt-cli/src/main.rs`: `FETCH_ENV` admits `http` when `origin` is a loopback host (cited)
- `crates/takt/src/forge/connectors.rs`, `crates/takt/src/forge/local.rs` and any other forge that pushes: the push env, the same rule (inferred)
- `crates/takt/src/harness.rs`: no credential for a loopback endpoint; an anonymous llm route accepted for loopback (cited)
- new: `crates/takt/tests/loopback_relays.rs` (inferred)

## Acceptance

1. With `origin` `http://127.0.0.1:<p>/o/r.git` served by a local smart-HTTP stand-in, a run
   fetches and pushes its branch; with `origin` `http://example.com/o/r.git` the run refuses the
   fetch, naming the URL.
2. `--harness harness --model-endpoint http://127.0.0.1:<p>/v1` with no credential flag starts its
   session; the stand-in sees no `Authorization` and no `x-api-key` header.
3. The same with `http://example.com/v1` is refused, as today.
4. No variable of the harness's environment holds a value the stand-ins were given as a token.
