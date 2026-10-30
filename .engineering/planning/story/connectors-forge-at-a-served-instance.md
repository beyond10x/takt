---
format: aep.planning-md/3
id: story:connectors-forge-at-a-served-instance
kind: story
status: active
title: The connectors forge opens and merges through a real served Connectors instance, with keyed writes
summary: connectors 0.62.0 served instance in the test and CI; idempotency_key on standing-grant writes
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:connectors-forge-speaks-github-at-an-endpoint
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T10:37:01Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T10:37:01Z", actor: "human:timo", revision: 3}
---
## Outcome

`forge: connectors` with `forge_endpoint` opens, reads and merges a GitHub pull request through a
real served Connectors instance (connectors 0.62.0), not only through a stand-in of its wire. On a
host with standing write grants, each write on `POST /v1alpha2/invoke` carries an
`idempotency_key` that is stable for the case's write, so a write repeated after a lost answer
replays its first outcome and is never applied twice.

## Context

Wave 47 (story:connectors-forge-speaks-github-at-an-endpoint) drives the served wire, but only
against a loopback stand-in in the test. Connectors 0.62.0 adds three things to a served instance:
- it admits a Kubernetes service account;
- it runs writes under a standing grant (`standing_grants`), opt-in;
- it relays git smart-HTTP.

On such a host, every write on `POST /v1alpha2/invoke` needs an `idempotency_key` and a covering
grant. Without one the write is `invalid_input` or `approval_required`. A key reused for another
request is `idempotency_conflict`, and a write a restart interrupted answers `outcome_unknown`. A
host without `standing_grants` dispatches unkeyed writes as before, and `idempotency_key` is
`unsupported` there. In a pod the caller reaches the instance through a loopback boundary that
adds the caller's bearer, so takt sends none.

## Acceptance

1. A test runs connectors 0.62.0's served instance on loopback, from its release asset, pinned by
   sha256. The instance's `github` connector points at a local fixture of GitHub's REST API, and a
   standing grant covers the target. Through it, a run on `forge: connectors`,
   `forge_instance: github` and `forge_endpoint: http://127.0.0.1:<p>` opens, reads and merges a
   pull request. The fixture saw exactly one create and one merge. CI installs the same asset and
   runs the test.
2. Each write takt sends on a standing-grant host carries an `idempotency_key` derived from the
   case and the write. The same case and write give the same key, and another write gives another
   key.
3. A write whose answer was lost and which is sent again with the same key replays: the fixture
   sees no second create or merge.
4. `idempotency_conflict`, `approval_required` and `outcome_unknown` each end the write with the
   instance's code named. `outcome_unknown` is followed by a read of the pull request before any
   further write (story:fenced-forge-dispatch).
5. Against a host without `standing_grants`, the writes go unkeyed as in 0.24.0, and the run
   still opens and merges.
