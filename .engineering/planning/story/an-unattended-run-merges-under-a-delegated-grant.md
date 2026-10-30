---
format: aep.planning-md/3
id: story:an-unattended-run-merges-under-a-delegated-grant
kind: story
status: active
title: An unattended run merges under a grant given by a delegated agent role
summary: no person's takt approve; a revocable owner delegation and a reviewer pass at the revision
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:36:01Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-10-29T21:36:02Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":4}}}
---
# Story: an-unattended-run-merges-under-a-delegated-grant

## Outcome

A run with no person present grants `repository.merge` itself, as the agent role
`agent:takt-delivery`, at the case's revision, when two things hold at the moment the frontier
finds the action approval-required: `.engineering/delegations.yaml` at the merge target's tip
records an unrevoked delegation of `repository.merge` (and, for a change to a judge surface,
`repository.merge.judge_surface`) to `agent:takt-delivery`, and the case holds validator evidence
at that revision for `tests.pass`, `gate.green` and `ci.green`. The grant is written to the case's
`authority.yaml` as `takt approve` writes one, naming `agent:takt-delivery`. Without the
delegation, or with any of the three claims missing or failed at the revision, nothing is granted
and the run stops `ApprovalRequired (repository.merge)` as today.

## Context

The merge validator merges only under a grant of `repository.merge` read from the case's
`authority.yaml`, at the revision it was given at (`crates/takt/src/validator.rs:4431-4460`).
Today the grant is `takt approve` by a person (`website/docs/guides/run-a-story.md:398`). A
change to a judge surface merges under the owner's grant, or an agent role's where
`.engineering/delegations.yaml` names an unrevoked delegation of
`repository.merge.judge_surface` to it (`validator.rs:4846-4852`, `:5056-5100`). In takt every
path but `docs/**`, `README.md` and `CHANGELOG.md` is a judge surface (`AGENTS.md` § Rules).

The owner chose option A of decision-blocker:unattended-merge-grant (cleared). The review the
grant rests on is the validators' evidence, never a model's text (`AGENTS.md` § Rules: evidence
comes from validators only): the tests at the revision, the gate at the revision, and the forge's
checks on the pull request's head, which is CI. A model reviewer step is not part of this story.
The delegation record in each target repository is the owner's commit, not takt's.

## Scope

- `crates/takt/src/run.rs`: where the loop meets `repository.merge` approval-required and stops
  (`Stop::ApprovalRequired`, `:12705`): grant first when the delegation and the three claims hold
  (cited). This is not the budget or harness-step region story:a-killed-step-records-its-tokens
  edits; both edit `run.rs`, accepted.
- `crates/takt/src/case.rs`: `grant` (`:1167`) by an agent role (cited)
- `crates/takt/src/validator.rs`: the delegation read (`:5056-5100`) generalised from
  `repository.merge.judge_surface` to `repository.merge` (cited). `PrMerged`'s merge itself is
  unchanged; story:fenced-forge-dispatch, which changes it, comes after this story.
- `crates/takt/tests/unattended_merge.rs`: new (inferred)

## Acceptance

In a fixture repository whose `.engineering/delegations.yaml` at the base delegates
`repository.merge` and `repository.merge.judge_surface` to `agent:takt-delivery`, with the
in-memory forge reporting the pull request's checks:

1. checks pass, tests and gate green: the run merges with no `takt approve`; the case's
   `authority.yaml` names `agent:takt-delivery` at the revision; a `merge_observation` of `merged`
   is written.
2. checks fail: the run stops `ApprovalRequired (repository.merge)`; `authority.yaml` holds no
   grant; the forge's call log has no merge call.
3. the delegation revoked (`revoked_at` set) at the base: as 2.
4. no delegations file: as 2.
