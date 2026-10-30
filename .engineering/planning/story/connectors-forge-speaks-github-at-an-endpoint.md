---
format: aep.planning-md/3
id: story:connectors-forge-speaks-github-at-an-endpoint
kind: story
status: draft
title: The connectors forge drives a GitHub pull request through a served instance at forge_endpoint
summary: github operations over /v1alpha2/invoke, no credential in takt
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:bot-forge-reads-through-the-app
revision: 1
---
# Story: connectors-forge-speaks-github-at-an-endpoint

## Outcome

`forge: connectors` with `forge_instance: github` drives a GitHub pull request through a served
Connectors instance: the run file's new key `forge_endpoint` names the instance's URL, takt calls
its served wire (`GET /v1/describe`, `POST /v1alpha2/invoke`) instead of the `connectors` CLI, and
holds no credential for it.

## Context

A takt pod with no secret in its container reaches the forge through a boundary container on
loopback (Loom's `spec.boundary`, `LOOM_BOUNDARY_CONNECTORS_URL`, `http://127.0.0.1:8402`); the
boundary relays to a Connectors instance that holds the GitHub App's key. Today
`crates/takt/src/forge/connectors.rs` maps only the gitlab connector's operations
(`merge_request.create/get/merge`, `pipeline.get`) and runs the `connectors` CLI.

The github connector's operations, from connectors-definitions `connectors/github.yaml`:
`branch.get`, `pull_request.create`, `pull_request.get`, `pull_request.merge`,
`check_runs.list` and `commit.status`, the last two for the head sha. GitHub has no pipeline id;
the checks pass only when every check run is `completed` with `success`, `neutral` or `skipped`
and the combined status is `success`, or `pending` with no statuses; they fail on any `failure`,
`cancelled` or `timed_out`; otherwise they are pending. `pull_request.merge` sends the head sha,
which GitHub checks (409 when the head moved).

Without `forge_endpoint` the CLI form stays as it is, for a run on the operator's machine.

Depends on the Connectors release that serves the github operations under a projected
service-account token, and on the Loom release with `spec.boundary`.

## Scope

- `crates/takt/src/forge/connectors.rs`: the github operation map, the served-wire client, the checks rule above (cited)
- `crates/takt/src/run_config.rs`: the `forge_endpoint` key, `http://` to a loopback host or `https://` (inferred)
- `crates/takt/src/forge.rs`: selection passes the endpoint (inferred)
- new: `crates/takt/tests/connectors_forge_github.rs`, against a loopback stand-in of the served wire (inferred)
- `ess/domains/takt.yaml`: the run file's `forge_endpoint` (inferred)

## Acceptance

1. A run on `forge: connectors`, `forge_instance: github`, `forge_endpoint: http://127.0.0.1:<p>`
   against a stand-in that answers the six operations opens, reads and merges a pull request, and
   the stand-in's log shows only `/v1/describe` and `/v1alpha2/invoke` requests, none with an
   `Authorization` header.
2. Check runs `[completed/success, completed/skipped]` with combined status `pending` and no
   statuses: pass. One `completed/timed_out`: fail. One `in_progress`: pending.
3. A merge the stand-in answers with 409 is no merge, and the run reads the pull request again
   before any further write.
4. `forge_endpoint: http://example.com` is refused with exit 2, naming the key and its line.
