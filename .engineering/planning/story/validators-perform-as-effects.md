---
format: aep.planning-md/3
id: story:validators-perform-as-effects
kind: story
status: implemented
title: Each validator performs as a Commission effect, and evidence enters only through submit_evidence
summary: Validators as EffectPorts; merge grant check, judge guard and record_before inside effects; a producer test over every fixture run
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:governor-holds-takts-cases
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T15:53:50Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T15:53:51Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-21T22:46:00Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: validators-perform-as-effects

## Outcome

Each validator of takt is an `EffectPort` the Commission runtime hands an admitted request, and the evidence it writes reaches the case only through `submit_evidence` of loom-sdk with the validator named as producer. No record reaches a case from anything but a validator, or a `takt.choice` record written by an effect.

## Context

- Today validators implement `Validator` (`crates/takt/src/validator.rs:46`); the loop calls them and writes their records. `record_before` (`crates/takt/src/run.rs:7434`) checks for a grant before a validator step and refuses with `StepRefused` (`run.rs:7730-7736`). The merge validator merges only under a grant of `repository.merge` at the case's revision, and is the one gate in Rust (`AGENTS.md:59-63`).
- In loom-sdk an `EffectPort` (Loom 0.26.0, `crates/loom-commission/src/ports/effect.rs:35`) is handed only a request the runtime revalidated against the revision, the frontier and the authority answer (`effect.rs:1-8`); its outcome reaches the governor as an observation, never as evidence. Evidence enters through `submit_evidence` (`crates/loom-commission/src/ports/evidence.rs:116`).
- The merge grant check, the judge guard and `record_before` move into the effects; no other gate is added. Evidence comes from validators only, with the choice-record exception (`AGENTS.md` § Rules).
- The edit effect of `story:harness-proposes-takt-performs` applies a proposal under the step guard; this story makes the validators the same kind of effect.

## Domain relations

- Step → EvidenceProducer: a deterministic step is performed by exactly one validator — inferable (`ess/domains/takt.yaml`, `takt.work.Step` relation `producer`).
- Run → EvidenceProducer: many — inferable (`ess/domains/takt.yaml`, `takt.work.Run` relation `validators`).

## Scope

- `crates/takt/src/validator.rs`, `crates/takt/src/evidence.rs`, `crates/takt/src/forge.rs` (each validator an effect; the merge grant check inside the merge effect)
- `crates/takt/src/run.rs` (`record_before` and the judge guard moved into the effects)
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv`
- new: `crates/takt/tests/effects.rs`

## Acceptance

1. Every validator-owned action of `story.delivery/1` and of the improvement protocols in `protocols/` is performed through an `EffectPort`; a test lists them against the actions each protocol gives a validator.
2. A producer test: over every fixture run in `crates/takt/tests`, every evidence record names a validator as producer, or is a `takt.choice` record an effect wrote; any other producer fails the test.
3. With an `AuthorityProvider` that allows every capability, the merge effect still refuses to merge without a grant of `repository.merge` at the case's revision; a test.
4. `step_cannot_forge_its_validators`, `ignored_directories_are_guarded*`, `judge_guard`, `judge_grant_bot` and `merge_under_authority` pass; every test whose expectation changed is named in the diff's description with its reason.
5. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass.

## Out of Scope

The loop that calls the effects (`story:the-loop-is-run-until-blocked`); confinement of the test command (`story:confinement-seam`).
