---
format: aep.planning-md/3
id: story:harness-endpoint-is-configured
kind: story
status: draft
title: takt --harness harness takes its endpoint and credential reference from flags or a file
relations:
- depends_on: epic:takt-on-loom-sdk
- informed_by: approval-record:trial-harness-arm-endpoint
revision: 2
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
