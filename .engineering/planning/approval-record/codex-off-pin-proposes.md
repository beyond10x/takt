---
format: aep.planning-md/3
id: approval-record:codex-off-pin-proposes
kind: approval-record
status: draft
title: A codex outside metaharness's pin is not a hard failure in takt
relations:
- decides: story:codex-off-pin-proposes
revision: 1
---
## Decision

Owner, 2026-10-27, relayed verbatim by the coordinating session: "Another defect is that currently
my current codex version is just not working together with takt because it, for whatever reason,
is doing some weird version check, which I think doesn't make much sense because we're using codex
via metaharness, and metaharness should just use that protocol from codex and just work. There
shouldn't be a hard failure due to that." And, 01:39Z: "make sure its tracked in the takt planning
store, and efficiently implemented and integrated".

The coordinating session, 2026-10-27 (its decision D65): "the takt pin goes onto integration/takt
at green and is pushed (fetch, merge origin/integration/takt, re-run affected suites, fast-forward
push, never force) ... No takt PR, no takt merge to main." All takt changes land on
`integration/takt` (owner, 2026-10-27).

## What is decided

`story:codex-off-pin-proposes` is accepted. Its pin lands on `integration/takt` once a loom-sdk
release takes metaharness 0.20.0; takt reaches metaharness only through
`b10x-loom-session-executor`'s exact tag, so that Loom release comes first.
