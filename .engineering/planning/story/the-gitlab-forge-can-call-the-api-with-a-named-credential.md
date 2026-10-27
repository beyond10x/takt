---
format: aep.planning-md/3
id: story:the-gitlab-forge-can-call-the-api-with-a-named-credential
kind: story
status: draft
title: The GitLab forge calls the API directly with a credential from a named source
summary: 'transport: api, credential from env or a mode-600 file, never in argv'
tags:
- priority:medium
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
- informed_by: story:gitlab-forge
scope:
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/forge/gitlab.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/tests
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 2
---
# Story: the-gitlab-forge-can-call-the-api-with-a-named-credential

## Outcome

The GitLab forge stays independent of Connectors and gains a second transport. Today it works through `glab`
(`crates/takt/src/forge/gitlab.rs`: `glab api --hostname`, push with `credential.helper=!glab auth git-credential`,
and takt holds no token).
- **The new transport.** `transport: api` calls the GitLab REST API over HTTPS directly. It covers merge request
  create, read and merge, and pipeline read for the request's head.
- **The credential.** It comes from a named source in the run file:
  - `credential: {env: NAME}`, or
  - `credential: {file: PATH}`: mode 600, owned by the user, outside the repository tree.

  The token value is never in the run file, argv, the state dir, an event or an error message. A source that is
  unset, readable by others or inside the tree is refused at open, naming the source, never the value.
- **Defaults.** The default transport stays `glab` on its own login. The push stays the scanned push. Over `api` it
  uses the same named credential through a credential helper that takt feeds on stdin, never in argv or the URL.

## Context

- Operator, 2026-10-27: a GitLab implementation independent of Connectors.
- A user's personal token must never be placed in configuration or argv. The credential is therefore a named source,
  and its value never leaves takt's memory.
- The seam is `Forge` from `story:scan-and-forge-are-adapters`. `story:gitlab-forge` is implemented (through `glab`),
  and `story:gitlab-forge-hardening` is a draft for the `glab` transport.

## Acceptance

Each is a named conformance scenario run by `task check`, against a fake GitLab server on localhost.

1. `gitlab_api::opens_reads_and_merges_through_the_api`: the fake records create, read and merge with the
   `PRIVATE-TOKEN` header equal to the token from `{env: NAME}`.
2. `gitlab_api::the_token_is_never_written`: after a full run, no file under the state dir or case dir, no recorded
   argv and no event contains the token bytes.
3. `gitlab_api::an_open_credential_file_is_refused`: `{file: PATH}` at mode 644, or inside the tree, is refused at open,
   naming the path.
4. `gitlab_api::a_missing_env_is_refused_by_name`: an unset `NAME` is refused at open, naming `NAME`.
5. `gitlab_api::checks_are_bound_to_the_head`: a pipeline for another head is not read as the request's checks.
6. `gitlab_api::glab_stays_the_default`: without `transport`, the run invokes `glab` as today.

## Scope

- `crates/takt/src/forge/gitlab.rs` (the `api` transport)
- `crates/takt/src/forge.rs` (`credential` source, redaction)
- `crates/takt/src/run_config.rs` (`transport`, `credential`)
- `ess/domains/takt.yaml` (the credential source, before code)
- `crates/takt/tests/` (fake server, conformance scenarios)

## Out of scope

- A live run against a real GitLab. It waits for the operator's identity choice.
