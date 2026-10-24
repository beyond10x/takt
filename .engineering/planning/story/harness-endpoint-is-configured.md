---
format: aep.planning-md/3
id: story:harness-endpoint-is-configured
kind: story
status: implemented
title: takt --harness harness takes its endpoint and credential reference from flags or a file
relations:
- depends_on: epic:takt-on-loom-sdk
- informed_by: approval-record:trial-harness-arm-endpoint
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/harness_endpoint_cli.rs
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T21:57:01Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-23T21:57:01Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-24T00:15:39Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

`takt --harness harness` takes its model endpoint (base URL, wire, model and the credential's
reference) from flags or a configuration file, so a run can use a credential that is not the local
Claude login. A credential can be a secret's name in the owner's secrets store (a namespace and a
name, the store backed by the OS keychain), which takt passes down by name and the harness
resolves; takt never reads the secret.

## Context

- The endpoint is a constant in `crates/takt-cli/src/main.rs:1398-1405` (at the wave 29 merge):
  `https://api.anthropic.com/v1`, `anthropic-messages`, `claude-opus-5`,
  `$HOME/.claude/.credentials.json` with pointer `/claudeAiOauth/accessToken`. A trial arm that runs
  `--harness harness` against another endpoint or with another credential cannot be configured
  without a code change.
- `Endpoint` (`crates/session-executor/src/lib.rs:52-65`) knows one credential kind: its
  `token_file` is always a subscription token, and the session maps it to the loop's subscription
  token file (`:316-318` and below). An API key is not a subscription token: on
  `anthropic-messages` a key goes in `x-api-key`, a subscription token as an OAuth bearer with
  Anthropic's OAuth headers.
- The owner decided on 2026-10-22 that the trial arm's API key is not stored in a file: it is a
  secret in the owner's secrets store, backed by the OS keychain, named by a namespace and a name,
  and the harness resolves it (approval-record:trial-harness-arm-endpoint). Resolving a secret's
  name is the harness's and metaharness's work; this story waits on their releases for
  acceptance 7.
- Scheduled after epic:takt-on-loom-sdk closes.

## Scope

- `crates/takt-cli/src/main.rs` (the `harness` arm and its flags), `crates/session-executor/src/lib.rs`
  (`Endpoint` and its mapping onto the session's run), `crates/takt/src/harness.rs`, the CLI
  reference the site generates, `Cargo.toml` and `Cargo.lock` for the metaharness release.

## Acceptance

1. `takt run --harness harness` with no endpoint flag and no configuration file builds the same
   `Endpoint` as before: a test compares the two.
2. Each of base URL, wire, model and credential can be set by a flag and by a configuration file;
   a flag wins over the file: a test per field.
3. `Endpoint`'s credential is one of two kinds: a subscription token file with an optional JSON
   pointer (today's), or a secret's name (namespace and name) for an API key. The subscription
   token maps to the loop's subscription token file as today; the secret's name is passed down as
   a name: a test per kind over the run the session builds.
4. With a secret's name, takt opens no secrets store and no keychain and reads no secret: the
   namespace and name are all takt passes and records; a test with no secrets store reachable
   from takt still builds the run.
5. No credential enters argv, the case record, the harness prompt or the harness's environment;
   the case records the subscription token file's path or the secret's name, never a secret: a
   test with a marker secret finds no marker in the case files.
6. A subscription token file that is missing, or whose pointer resolves to nothing, fails before
   the first step with a message naming the path and the pointer, exit non-zero: a test.
7. Two configured endpoints, each with the secret's name namespace `llm`, name
   `opencode-go-api-key`, build the request they name, each a test over what takt hands the
   harness (base URL, wire, model, the secret's name):
   - wire `anthropic-messages`, base `https://opencode.ai/inference/go/anthropic/v1`, model
     `claude-haiku-5-5`;
   - wire `openai-responses`, base `https://opencode.ai/inference/go/openai/v1`, model
     `gpt-6-luna`.

## Delivered

Wave 36 (docs/waves/wave-36.md): U3 at 8c171e0, U3b at 1782919, the review's fixes at db65aad,
U3c at 731d111 on loom 0.39.1. `--harness harness` takes its endpoint from the `--endpoint-*`
flags or `--endpoint-file` (TOML), a flag winning over the file, and with neither builds the
endpoint as before (acceptance 1, 2); the credential is a subscription token file with an
optional JSON pointer, mapped as today, or a secret's name, handed to b10x-harness as
`SessionExecutor::api_key_secret` with the endpoint naming no token (3); takt opens no store
and reads no secret (4); the case's `opened` event records base URL, wire, model and the
credential's reference, the token file from `~` under HOME, never a token, and a marker test
finds none in the case files (5); a missing token file or an empty pointer exits 1 before the
first step naming both (6); the two trial endpoints (`anthropic-messages` on
`https://opencode.ai/inference/go/anthropic/v1` with `claude-haiku-5-5`, `openai-responses` on
`https://opencode.ai/inference/go/openai/v1` with `gpt-6-luna`, both `llm/opencode-go-api-key`)
each build what takt hands the harness (7). The review's F2, F3, F7 and F10 narrowed it: the
default credential goes to the default base URL alone, `http://` to loopback only, no userinfo,
query or fragment, a relative token file made absolute. Not done in takt: a real run against
either trial endpoint, which the trial agent starts from the release that carries this.
