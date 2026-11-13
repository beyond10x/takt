---
format: aep.planning-md/3
id: story:forge-connectors-docs-name-the-bearer-boundary
kind: story
status: draft
title: 'The forge: connectors documentation names the boundary that adds the bearer'
summary: the reference and README say the served route works with no credential; its test adds the bearer on loopback
tags:
- finding
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: forge-connectors-docs-name-the-bearer-boundary

## Outcome

The documentation of `forge: connectors` says what the served route needs between takt and a served Connectors
instance: takt sends no credential, the served host admits a request only with a bearer token, and the route works
through a boundary on loopback that adds it, as takt's own test of the route runs it.

## Context

Found by the review of pull request 68 (the 0.36.0 release prepare), outside its diff. `forge: connectors` runs on the
same client as the connector action (`crates/takt/src/connectors_client.rs:5-7`), which sends no `Authorization`
header. A served Connectors host refuses a request without `Authorization: Bearer` (beyond10x/connectors
`crates/connectors-host/src/server.rs`; the catalog provider goes through the same check).

Two places say the route works against a served instance and name nothing in between:

- `website/docs/reference/configuration.md:558`: "The served route is tested against a served instance of
  Connectors 0.62.0 (`connectors-catalog-provider --config`) with standing grants".
- `README.md:49`: "`forge: connectors` drives a GitHub pull request through a served Connectors instance at
  `forge_endpoint` with no credential".

The test both rest on, `crates/takt/tests/connectors_served_instance.rs:15-17`, puts "the loopback boundary a pod
reaches the instance through" in front of the instance: "it adds the service bearer to every request, since takt sends
none". So the claim holds with that boundary, and a reader who points `forge_endpoint` at the instance itself gets
HTTP 401. The 0.36.0 reference states the same limit for `connector_actions` in a caution; that caution says no
connector action has been run against a served host, while this test shows the client reaching one through the
boundary.

## Acceptance

1. `website/docs/reference/configuration.md` (the `forge: connectors` section) and `README.md` say that takt sends no
   credential, that a served Connectors host refuses a request with no bearer token, and that the route is run through
   a boundary on loopback that adds it, citing the test that runs it so.
2. The `connector_actions` caution names the same boundary as the route that reaches a served host.
3. A predicate against a served host, when one can run: with `forge_endpoint` at the served instance itself, the open
   is refused with HTTP 401 and nothing is written; through the boundary, it opens. Not required for this draft.
