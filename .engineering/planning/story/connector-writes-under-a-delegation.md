---
format: aep.planning-md/3
id: story:connector-writes-under-a-delegation
kind: story
status: draft
title: takt grants itself a connector write under the owner's scoped delegation
summary: connector.write delegated by record, one action id and an expiry, revocable
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
- depends_on: story:a-connector-write-under-a-grant
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: crates/takt-cli/tests/connector_delegation_cli.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/run.rs
revision: 5
---
# Story: connector-writes-under-a-delegation

## Outcome

An unattended reconcile writes without a person per tick: under an owner's delegation of `connector.write` in
`.engineering/delegations.yaml`, scoped to one action id and an expiry and naming a role, takt grants itself the write
per case as `agent:<role>` when the delegation holds at the tip the forge reports and the render's record is `pass` at
the revision, as `Run::grant_delegated` does for `repository.merge` (`crates/takt/src/run.rs`). A revoked or expired
delegation grants nothing.

## Context

Design § 5, option B. Blocked by decision-blocker:connector-writes-unattended: AGENTS.md § Rules says "Add no other
path by which takt grants itself"; this story is that path and widens authority.

## Acceptance

- Under a delegation: the write is granted and sent with no `takt approve`: a test.
- Revoked, expired, for another action id, or not at the tip: exit 4, nothing sent: tests.

## Scope

- `crates/takt/src/run.rs` (`grant_delegated`); `crates/takt/src/case.rs`; `AGENTS.md` § Rules; new
  `crates/takt-cli/tests/connector_delegation_cli.rs`.
