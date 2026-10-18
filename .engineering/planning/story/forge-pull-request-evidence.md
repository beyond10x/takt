---
format: aep.planning-md/3
id: story:forge-pull-request-evidence
kind: story
status: draft
title: Open the pull request through the bot route and read its state as evidence
summary: The Forge seam over b10x-gates; pr.opened and ci.green only from what the forge reports
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:story-delivery-protocol
- depends_on: story:gate-validator
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/git.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/forge.rs
revision: 5
---
# Story: forge-pull-request-evidence

## Outcome

When a case's implementation revision has its tests and gate green, the run pushes it and opens one pull request through the repository's bot route, and `pr.opened` and `ci.green` become true only from what the forge reports about that revision, never from what the harness says.

## Context

`docs/design/takt.md` § 3 Seams: `Forge (pull request, merge, release through the repository's bot route)`; § 3 Shape: deterministic steps are "a command (a gate, a pull request, a CI read, a merge on approval)". `AGENTS.md:32-34`: every commit, push, pull request and tag goes through `b10x-gates bot -- <git args>`, API writes through `b10x-gates api`, and pull-request bodies pass `b10x-gates scan-text` first. Today the kernel commits locally with a fixed identity (`crates/takt/src/git.rs:44-61`) and never pushes, branches or talks to a forge.

This story adds the `Forge` trait with one adapter that shells out to `b10x-gates` (the bot credential never enters takt): `push(branch)`, `open_pull_request(branch, title, body)`, `pull_request(number)` giving open, merged or closed, `checks(sha)` giving pass, fail or pending. Two validators are built on it, in the shape `TestsRun` has (`crates/takt/src/validator.rs:19-31`): `pr.open` performs the push and the open and writes a `pull_request` record with result `open`; `ci.read` writes a `ci_result` record with result `pass` or `fail` and writes nothing while checks are pending. One branch and one pull request per case, the branch named from the case id and created when the case opens: the claims are about `implementation` at a revision (story:story-delivery-protocol), so the pull request head is that revision. The pull-request body is generated from the case (intent, the claims that hold) and passed through `scan-text` before it leaves the machine. The `gate_result` record that makes `gate.green` true, a precondition of `pr.open`, is written by the `gate.run` validator of story:gate-validator.

## Domain relations

- Validator → Action, EvidenceKind, Subject: exactly one each. Inferred from `crates/takt/src/validator.rs:20-31`.
- EvidenceRecord → artifact revision: one subject at one revision. Inferred from b10x-canon 0.6.0 `crates/canon/src/model/evidence.rs:30-35`.
- Case → Protocol: exactly one. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:644-651`, entity `canon.protocol.Case`, relation `protocol`.

## Scope

- new: `crates/takt/src/forge.rs` (the trait, the `b10x-gates` adapter, an in-memory test double)
- `crates/takt/src/lib.rs` (the `forge` module registered)
- `crates/takt/src/git.rs` (branch at case open, push through the bot route)
- `crates/takt/src/validator.rs` (`PrOpen`, `CiRead`)
- `crates/takt/src/case.rs` (the branch recorded in the case directory)
- `crates/takt-cli/src/main.rs` (forge selection, `--forge none` for intent runs that never leave the machine)
- new: `crates/takt/tests/forge.rs`

## Ordering

- depends_on story:story-delivery-protocol: this story's runs read `protocols/story-delivery/1.yaml`; no shared file.
- depends_on story:gate-validator: both edit `crates/takt/src/validator.rs` and `crates/takt-cli/src/main.rs`, and the acceptance reads the `gate_result` record that story's validator writes.

## Acceptance

With a forge test double reporting the pull request open and its checks passing, a run on `protocols/story-delivery/1.yaml` with `--harness none` and the tests and gate validators green writes a `pull_request` record with result `open` and a `ci_result` record with result `pass`, both about `implementation` at the pushed revision, and its decision shows `pr.opened` and `ci.green` true and `repository.merge` approval-required, while the same run with the double reporting checks pending writes no `ci_result` record and shows `ci.green` unknown.

## Out of Scope

The merge (story:merge-under-authority); releases and tags; a second forge; reading CI for anything but the pull request head.

## Ambiguities

- Branch name and pull-request title: `inferable` as recorded here, the case id and the intent title; nothing in the design constrains them.
- One pull request per case: `inferable` as a consequence of binding the claims to `implementation` (story:story-delivery-protocol); a case with two pull requests would have two heads for one revision line.
- The real adapter is proven on a scratch repository the bot route can reach; the test double proves the loop.
