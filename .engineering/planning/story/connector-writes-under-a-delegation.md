---
format: aep.planning-md/3
id: story:connector-writes-under-a-delegation
kind: story
status: draft
title: takt grants a connector write the repository declares, with no takt approve
summary: connector.write delegated by record, one action id and an expiry, revocable
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
- depends_on: story:a-connector-write-under-a-grant
scope:
- confidence: cited
  path: crates/takt-cli/tests/connector_declaration_cli.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/connector_action.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 13
---
# Story: connector-writes-under-a-delegation

## Outcome

A repository declares the connector writes it allows in its `.engineering/delegations.yaml`, and takt makes them with
no `takt approve`. An entry names the capability `connector.write`, the role takt grants as, the connector and its
instance, the operations, the target as JSON pointers and values into the write's input (for a chat, `/channel` and
one channel id), an optional `max_per_hour` and `not_after`, and `revoked_at` once revoked. takt reads the file at the
revision the case opened on; when a write's operation and target fall inside an entry that is not revoked, not
expired and not over its rate, takt records the grant in the case's `authority.yaml` as `agent:<role>`, naming the
entry, and the write is sent. Any other write is refused, nothing is sent, and the refusal names the entry it missed.

## Context

Design § 5; decision-blocker:connector-writes-unattended, cleared by the owner on 2026-11-12: "in our specific case,
there should be no approval, ideally this behavior/permissions can be configured via config file". `AGENTS.md` § Rules
names this path. It mirrors the merge's delegation (`Run::grant_delegated`, `crates/takt/src/run.rs`): the grant is
written by takt and decided by the governor and `CaseAuthority`, never by a check inside the validator. The rate is
counted from the grants recorded in the cases under the repository's cases directory within the last hour. Depends on
story:a-connector-write-under-a-grant.

## Acceptance

- `declared_write_needs_no_approval`: an entry covering `chat.postMessage` to one channel: the write is sent once, the
  case's `authority.yaml` holds the grant as `agent:<role>` naming the entry, and the run exits 0 with no
  `takt approve`.
- `undeclared_operation_refused`: the same entry and a `chat.delete` write: refused, nothing sent, the refusal names
  the operation.
- `undeclared_target_refused`: the same entry and a write to another channel: refused, nothing sent.
- `revoked_or_expired_entry_grants_nothing`: `revoked_at` set, or `not_after` past: exit 4, nothing sent.
- `rate_is_held`: `max_per_hour: 1` and two cases in the hour: the second is refused, nothing sent.
- `declaration_read_at_the_case_revision`: an entry added to the tree after the case opened, uncommitted or at a later
  commit, grants nothing to that case.
- `declaration_refusals`: an entry missing `operations` or `target`, or naming another capability's fields, is refused
  when read, naming the file and line, exit 2.
- Spec first: `takt.work.ConnectorWriteDeclaration` in `ess/domains/takt.yaml`; `ess specify validate --path ess`
  exits 0.

## Scope

- `crates/takt/src/run.rs` (the grant); `crates/takt/src/case.rs` (the delegation file's new entry kind);
  `crates/takt/src/connector_action.rs`; `ess/domains/takt.yaml`; new
  `crates/takt-cli/tests/connector_declaration_cli.rs`; `website/docs/reference/configuration.md` (by the doc-writer).
