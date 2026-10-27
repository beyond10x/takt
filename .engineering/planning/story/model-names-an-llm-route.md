---
format: aep.planning-md/3
id: story:model-names-an-llm-route
kind: story
status: implemented
title: --model names an llm route; llm resolves endpoint, wire and model
summary: a route or alias fills the session's endpoint; its credential reaches the session by reference only
tags:
- config-proxy
relations:
- decomposes: epic:run-config-proxies-llm-and-metaharness
- serves: vision:safe-agentic-coding
- depends_on: story:llm-section-is-an-llm-routes-document
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/tests
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:36:41Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-10-27T01:36:41Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "active", to: "implemented", at: "2026-10-27T08:29:26Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":5}}}
---
# Story: model-names-an-llm-route

## Outcome

With an `llm` section and `--harness harness`, `--model <name>` (or the run file's `model`) names a route or alias of
that document; llm resolves it to the endpoint, wire and upstream model the b10x-harness session needs, and the route's
credential reaches the session by reference only.

## Context

- The owner, 2026-10-27: "configure model routes in my config" (epic:run-config-proxies-llm-and-metaharness).
- Today one endpoint per run is typed by takt: base URL, two wires, model default `claude-opus-5`, token file, pointer,
  secret or variable (`crates/takt/src/harness.rs:2603-2671`, `:2711-2719`, `:2806`); it becomes Loom's
  `session::Endpoint` (`harness.rs:575-577`) and is pointed at b10x-harness only, never at Claude Code or Codex
  (`harness.rs:491`). The run's endpoint is recorded on the `opened` record as `takt.work.HarnessEndpoint`
  (`ess/domains/takt.yaml:1101`).
- llm 0.16.0 resolves a name to a route or alias, else `<endpoint>/<model-id>` (`crates/llm-models/src/routes.rs:1675-1698`),
  and exposes each endpoint's base URL, protocol and secret kind but never the secret's variable, path or value
  (`:685-714`). It resolves the secret itself through `Routes::resolvers(&SecretSources)` (`:1852-1867`).
- takt already stages a credential variable as a protected token file that reaches no child
  (story:endpoint-credential-from-environment).

## Decision

- The run's `model` (flag or file key, `crates/takt/src/run_config.rs:83`) is resolved against `llm` when the run has
  an `llm` section. The route's endpoint base URL, protocol and upstream model fill the session's endpoint; `messages`
  maps to `anthropic-messages`, `responses` to `openai-responses`; a route on `chat-completions` or `bedrock-converse`
  is refused, naming the route and the protocol, until metaharness carries it.
- One endpoint source per run: an `llm` section beside any other endpoint source (`--endpoint-file`, the file's
  `endpoint_file`, an `--endpoint-*` flag, or the `--model-endpoint` family of story:run-flags-spell-metaharness-and-llm)
  is refused before any case opens, naming both sources; neither silently wins.
- An `llm` section with `--harness claude` or `--harness codex` is refused before any case opens, naming the harness:
  takt points an endpoint at b10x-harness only.
- The credential is resolved by llm's resolver inside takt and staged as the protected token file takt already uses;
  argv, the child's environment, the prompt and the case hold no value.
- Spec first: `takt.work.RouteName`, a newtype of String, is what `model` names when an `llm` section is present.
- Order: after story:llm-section-is-an-llm-routes-document, whose parsed section it reads.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check` with `--harness harness`
against a loopback endpoint.

1. `route/alias-resolves-to-endpoint`: `--model fast`, an alias of route `r` on endpoint `e`, gives an `opened` record
   whose `endpoint` is `e`'s base URL, the wire `e`'s protocol maps to and `r`'s upstream model, and the loopback
   endpoint receives a request for that upstream model.
2. `route/unknown-name-refused`: a `model` that is no route, alias or declared `<endpoint>/<model-id>` exits 1 before
   any case opens, with llm's message.
3. `route/unsupported-protocol-refused`: a route on `chat-completions` exits 1 before any case opens, naming the route
   and the protocol.
4. `route/credential-by-reference`: a route whose `auth.secret` is `env:VAR` reaches the loopback endpoint with the
   variable's value, and that value is in no argv, no child environment and no case file.
5. `route/llm-section-other-harness-refused`: an `llm` section with `--harness codex` exits 1 before any case opens,
   naming `codex`.
6. `route/no-llm-section-endpoint-file`: without an `llm` section, `--endpoint-file` gives an `opened` record whose
   `endpoint` holds the file's base URL, wire and model, and the existing endpoint tests pass unchanged.
7. `route/two-endpoint-sources-refused`: an `llm` section with `--endpoint-file e.toml`, and again with
   `--endpoint-base-url <url>`, exits 1 before any case opens, naming the `llm` section and the other source.

## Out of Scope

- Fallback across a route's targets: `llm.routes/1` expands to single-target routes (llm 0.16.0 `routes.rs:1436-1445`).
- Pointing Claude Code or Codex at a route's endpoint.
- Pricing a step from the route's `price`.

## Scope

- `crates/takt/src/harness.rs` (endpoint from a route; `EndpointConfig`), `crates/takt-cli/src/main.rs` (`harness_of`),
  `crates/takt/tests/`, `ess/domains/takt.yaml` (`RouteName`, `HarnessEndpoint`), `ess/ess-inputs.yaml`,
  `website/docs/reference/configuration.md`.

## Conformance acceptance

This story's acceptance names conformance scenarios in `ess/ess-inputs.yaml`, run by `task check`. None is run: that file
lists no scenarios (`scenarios: []`) and this repository has no conformance runner (`Taskfile.yml` names no conformance
step). The behaviour is checked by the cargo tests in `crates/takt-cli/tests/run_config_llm*.rs`, `run_config_llm_env_scrub.rs`
and the unit tests of `crates/takt/src/run_config.rs` and `crates/takt/src/harness.rs`, as the Status sections say. The
conformance acceptance stays open under `approval-record:cfgproxy-end-wave-review-and-conformance` until a later wave builds the
runner.
