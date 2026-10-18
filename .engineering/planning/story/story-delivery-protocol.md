---
format: aep.planning-md/3
id: story:story-delivery-protocol
kind: story
status: active
title: A protocol/1 document for delivering a planning-store story
summary: Claims tests.pass, gate.green, pr.opened, ci.green, pr.merged and outcome accepted, as data Canon validates
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/tests/story_delivery.rs
- confidence: inferred
  path: protocols/story-delivery/1.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T02:34:09Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T02:34:09Z", actor: "human:timo", revision: 5}
---
# Story: story-delivery-protocol

## Outcome

An operator who opens a case on a planning-store story can read from the decision document which of `tests.pass`, `gate.green`, `pr.opened`, `ci.green` and `pr.merged` hold at the current implementation revision, and whether `accepted` is legitimate, with Canon deciding and nothing in Rust modelling the ladder.

## Context

`docs/design/takt.md` § 4 M2 names the claims and the outcome. The built-in `software.change/1` (b10x-canon-engineering 0.9.0, `protocols/software-change/1.yaml`) ends in `accepted` only with `objective.realized`, `deployment.healthy` and `release.proven`, which a story in a repository with no deployment cannot earn, and it declares no pull request, CI or gate evidence. A story needs its own protocol, and `AGENTS.md:27-28` says a protocol is data: a built-in or a `protocol/1` file, never a model in Rust. The kernel already loads a file: `LoadedProtocol::from_file` (`crates/takt/src/protocol.rs:43-47`), selected with `--protocol <path>` (`crates/takt-cli/src/main.rs:33-35`, `crates/takt/src/run.rs:96-100`).

The document this story writes, validated and compiled by Canon exactly as the built-ins are:

- artifacts `intent`, `plan`, `implementation`; no case input (a declared input nothing reads refuses every caller that does not supply it; Canon takes the default when the key is left out: adversary pass 1, finding 2);
- evidence kinds `test_result` (pass, fail), `gate_result` (pass, fail), `pull_request` (open), `ci_result` (pass, fail), `merge_observation` (merged);
- claims `tests.pass`, `gate.green`, `pr.opened`, `ci.green`, `pr.merged`, every one about `implementation` at its current revision, so a new commit returns all of them to UNKNOWN the way the kernel already relies on (`crates/takt/src/case.rs:119-128`, `run.rs:250-254`);
- actions `repository.inspect`, `tests.write`, `implementation.edit`, `tests.run` with the ids and effects of `software.change/1`, so the harness brief (`crates/takt/src/harness.rs:71-81`) and the tests validator (`crates/takt/src/validator.rs:56-58`) apply unchanged; `gate.run`; `pr.open` with precondition `tests.pass` and `gate.green`; `ci.read`; `repository.merge` with precondition `ci.green` and `pr.opened`, requiring capability `repository.merge`;
- outcome `accepted` requiring `pr.merged`; `abandoned` on an explicit decision.

## Domain relations

- Case → Protocol: exactly one, referenced. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:644-651`, entity `canon.protocol.Case`, relation `protocol` (references, cardinality one).
- EvidenceRecord → artifact revision: one subject at one revision; a record about another revision is excluded. Inferred from b10x-canon 0.6.0 `crates/canon/src/model/evidence.rs:30-35` and `crates/takt/src/evidence.rs:13-14`.

## Scope

- new: `protocols/story-delivery/1.yaml` (the `protocols/<name>/<major>.yaml` layout of b10x-canon-engineering 0.9.0)
- new: `crates/takt/tests/story_delivery.rs`
- `crates/takt/src/protocol.rs` (only if a repository-local lookup by id is added beside the path)
- `README.md` (the protocol named beside the built-ins)

## Acceptance

A test opens a case on `protocols/story-delivery/1.yaml` through `CaseDir::open`, adds evidence records about `implementation` at its revision in the order `test_result` pass, `gate_result` pass, `pull_request` open, `ci_result` pass, `merge_observation` merged, and the decision after each record shows exactly one more claim true, `repository.merge` approval-required before the last record, and `accepted` legitimate only after it.

## Out of Scope

The validators that produce these records (story:gate-validator, story:forge-pull-request-evidence, story:merge-under-authority); contributing the protocol upstream to the engineering protocols; code review or security review obligations.

## Ambiguities

- Where the protocol lives: `inferable`, `AGENTS.md:27-28` admits a `protocol/1` file in this repository; upstreaming is a later choice and not this story.
- What the pull request claims are about: `inferable` as a design choice recorded here, `implementation` at a revision, because every claim the kernel tracks is bound to that artifact (`crates/takt/src/run.rs:254`); a separate `pull_request` artifact would need its own revision bookkeeping and was not chosen.

## Correction (wave 01)

Canon 0.6.0 reads a kind whose records disagree at one revision as unknown (`website/docs/reference/evaluation.md:39-42`), so `pr.merged` is established by a `merge_observation` record with result `merged`, not by a second `pull_request` result; `protocols/story-delivery/1.yaml` declares it so.
