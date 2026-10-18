---
format: aep.planning-md/3
id: story:forge-pull-request-evidence
kind: story
status: implemented
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
  path: crates/takt-cli/tests/forge_cli.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/git.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/forge.rs
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T05:47:50Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-18T05:47:50Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-18T07:20:05Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1,"review_outcome":3}}}
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

## ## Scope

## Scope

Confirmed by the implementor (wave 04), each line as the unit landed:

- `crates/takt/src/forge.rs` (inferred → confirmed): created; the `Forge` trait, the `BotRoute`
  adapter over `b10x-gates` and `gh` through a `Runner` seam, `Origin` from the remote URL, the
  `Memory` double.
- `crates/takt/src/lib.rs` (inferred → confirmed): the module and its re-exports.
- `crates/takt/src/git.rs` (cited): confirmed; `current_branch`, `create_branch` from a given
  revision, `leave_branch`, `remote_url`.
- `crates/takt/src/validator.rs` (cited, corrected: `TestsRun` sat at :54-132 in the base, not
  :19-31): `PrOpen` and `CiRead`.
- `crates/takt/src/case.rs` (cited): confirmed; `Branch` in `branch.yaml` (name, base, left,
  pushed, pull_request), the case branch forked from its base at open, `current`, `discard`.
- `crates/takt-cli/src/main.rs` (cited): confirmed; `--forge bot|none`, the bot route by default
  only for an origin on github.com.
- `crates/takt/tests/forge.rs` (inferred → confirmed): created; 123 cases, 21 of them the
  adversary's.
- `crates/takt/src/run.rs` (added by the coordinator's patch): a refused frontier candidate is
  discarded whole.
- `crates/takt-cli/tests/forge_cli.rs` (added by the adversary, adopted by the unit): the binary
  end to end on scratch repositories.
