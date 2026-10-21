---
format: aep.planning-md/3
id: story:governor-holds-takts-cases
kind: story
status: active
title: The Loom governor holds and decides takt cases; grants come from an AuthorityProvider over authority.yaml
summary: CanonGovernor over a takt FallibleCaseStore on .engineering/cases/<id>/; case files written as before; the grant change is the owner's call at the verdict
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:harness-proposes-takt-performs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T14:44:43Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T14:44:43Z", actor: "human:timo", revision: 3}
---
# Story: governor-holds-takts-cases

## Outcome

Every decision of a takt case is taken by the governor of Loom over a case store that keeps the case where takt keeps it today, `.engineering/cases/<id>/`, and a grant reaches the run through an `AuthorityProvider` that reads the case's `authority.yaml`. A reader of the case directory finds the files it finds today, plus the observations the Commission runtime records.

## Context

- Today `CaseDir::evaluate` (`crates/takt/src/case.rs:2323-2355`) hands Canon the authority itself (`case.rs:2339-2347`), each grant only at the revision of its subject it was given at (`case.rs:2318-2320`, `case.rs:564-570`).
- The governor of Loom supplies no authority (Loom 0.26.0, `crates/loom-governor/src/lib.rs:54-56`, `:1742-1744`). The Commission runtime asks an `AuthorityProvider` instead (`crates/loom-commission/src/ports/authority.rs:21-29`), at the moment of the request. The grant therefore moves out of the evaluation into an authority answer, and a decision taken after a grant reads `approval-required` for that action.
- That changes who decides a grant, and touches the rule that the merge validator is the one gate in Rust (`AGENTS.md:59-63`). The owner decides the change at this story's verdict, not before. The diff amends `AGENTS.md` § Rules to say where the gate is decided, and its merge needs the owner's grant.
- The governor holds a case through a `FallibleCaseStore` (`crates/loom-governor/src/lib.rs:373`); takt implements one over `.engineering/cases/<id>/`. The governor admits one capability per action (`crates/loom-governor/src/lib.rs:1376-1384`); the test names each protocol action of takt it checks against that.
- Records stay comparable for promotion: a scorecard over cases written before this story and after it reads the same files.

## Domain relations

- Commission case → takt case: one-to-one, the case directory holds both — inferable (`ess/domains/takt.yaml`, `takt.work.Case` identity `id`; inferred from Loom 0.26.0 `crates/loom-governor/src/lib.rs:348-360`, a case store keyed by case id).
- Case → authority grant: one-to-many, at most one grant per capability, the case owns its `authority.yaml`, each grant bound to the revision it was given at — inferable (inferred from `crates/takt/src/case.rs:564-570`).
- Case → decision: one per evaluation, owned by the case — inferable (inferred from `crates/takt/src/case.rs:2349-2353`).
- Run → Case: one — inferable (`ess/domains/takt.yaml`, `takt.work.Run` relation `case`).

## Scope

- `crates/takt/src/case.rs` (the case store, the authority provider, evaluation through the governor)
- `crates/takt/src/protocol.rs` (the grant subjects the provider reads)
- `crates/takt/src/run.rs` (the call site of evaluation)
- `crates/takt/src/loop_scorecard.rs` (reads `decisions/`)
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv`
- `AGENTS.md` (§ Rules, where the merge gate is decided)
- new: `crates/takt/tests/governor_case_store.rs`

## Acceptance

1. A fixture case run to its merge gate writes `case.yaml`, its evidence, `decisions/NNNN.json` and `authority.yaml` where it writes them today, plus `observations.json`; each `decisions/NNNN.json` is the `canon-decision/1` document the governor rendered for that evaluation.
2. `merge_under_authority`, `authority*`, `grant_subject*`, `judge_guard` and `judge_grant_bot` pass; every test whose expectation changed is named in the diff's description with its reason.
3. A grant of `repository.merge` given at one revision of its subject is refused at another; the `grant_subject` tests hold it unchanged.
4. A case left at its merge grant by the last release before this story resumes after `takt approve` and merges.
5. `loop_scorecard` reads the decisions it read before, and its tests pass.
6. `AGENTS.md` § Rules says where the merge gate is decided.

## Blocked by

- The governor of Loom exposes the `canon-decision/1` document behind each frontier and completion it determines; today it renders the document and keeps only its hash (Loom 0.26.0 `crates/loom-governor/src/lib.rs:1141`). Without it no `decisions/NNNN.json` is written (acceptance 1 and 5).
- `AuthorityProvider::decide` receives the request it is asked about — the action, the case revision and the revisions of the artifacts it is about; today it receives the commission and the capability only (`crates/loom-commission/src/ports/authority.rs:21-29`). Without it a grant cannot be bound to a revision (acceptance 3).

## Out of Scope

Validators as effects (`story:validators-perform-as-effects`); the loop (`story:the-loop-is-run-until-blocked`); removing `CaseDir::evaluate` (`story:takt-evaluation-code-retired`).
