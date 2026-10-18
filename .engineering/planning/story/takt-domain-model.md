---
format: aep.planning-md/3
id: story:takt-domain-model
kind: story
status: implemented
title: Declare takt's nouns and their relations as an ESS domain document
summary: Case, Run, Workflow, Step and EvidenceProducer typed and validated; the story's case and the authority grant as decided; no UNMAPPED marker
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/system.yaml
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T02:34:09Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-18T02:34:09Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-18T03:21:54Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":4,"verification":1}}}
---
# Story: takt-domain-model

## Outcome

A reader of this repository can check with `ess specify validate` which relations hold between a case, a run, a workflow, a step and an evidence producer, including how a story relates to its case and how authority is granted, instead of reading them out of story bodies.

## Context

`AGENTS.md:37` says this repository has no specification yet. The design (`docs/design/takt.md` § 3 Shape, Building blocks, Seams) introduces nouns no ESS document in this repository declares: Run, Unit (an intent or a planning-store story), Workflow and its blocks (protocol, step, loop, gate, approval), and the seams Harness, Frontier, Validator, Forge and Confinement, with the Validator as the evidence producer. The epic's Why names the nouns takt introduces: case, run, workflow, step, evidence producer. Canon declares the nouns it owns (Protocol, Case, EvidenceRecord, Decision, AuthorityDecision) in its own ESS domain, `ess/domains/protocol.yaml` of b10x-canon 0.6.0 (the tag `Cargo.toml` pins); this story declares takt's Case as the case a run opens, referencing `canon.protocol.Case`, and does not redeclare Canon's entities.

The planning skill asks that a noun with no typed home be modelled before stories are written around it. The sibling stories of this epic were drafted with code citations marked `inferred` instead, because nothing else existed; this document is where those inferences become declarations. Nothing is left undecided: the one open question, decision-blocker:approval-grant-and-case-resume, is cleared, so the document carries no `UNMAPPED:` marker.

## Domain relations

- Case → Protocol: exactly one; the case references the protocol; the protocol exists before and outlives the case. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:916-923`, entity `canon.protocol.Case`, relation `protocol` (kind references, cardinality one).
- Run → Case: one-to-one today, inferred from `crates/takt/src/run.rs:75` and `run.rs:105`; the outer loop (story:declared-loops) makes it one-to-many, one case per unit.
- Run → Unit: one intent, required, inferred from `crates/takt/src/run.rs:21-22` and `crates/takt-cli/src/main.rs:30-32`.
- Story → Case: at most one open case per story; a story is taken when `proposed` and is `active` while its case is open (aep `artifacts/lifecycles/story.yaml`, `transitions`, `descriptions`); a later run resumes that case rather than opening a second. Decided in the cleared decision-blocker:approval-grant-and-case-resume.
- AuthorityDecision → Case: one list per evaluation, at most one decision per capability, inferred from b10x-canon 0.6.0 `crates/canon/src/eval/mod.rs:359-365` and `crates/canon/src/eval/authority.rs:7-21`; written by `takt approve <case> <capability>` as the case's `authority.yaml` and read into every evaluation of the resumed case. Decided in the cleared decision-blocker:approval-grant-and-case-resume.
- Validator → Action, EvidenceKind, Subject: exactly one each, inferred from `crates/takt/src/validator.rs:20-31`.
- Harness step → Action: exactly one of the offered actions, inferred from `crates/takt/src/run.rs:188-193` and `crates/takt/src/harness.rs:42-47`.

## Scope

- new: `ess/domains/takt.yaml`, `ess/system.yaml`, `ess/ess-inputs.yaml` (the layout b10x-canon 0.6.0 and b10x-canon-engineering 0.9.0 use: `ess/domains/`, `ess/system.yaml`, `ess/ess-inputs.yaml`)
- `AGENTS.md` (the rule that says no specification exists, line 37)

## Ordering

No `depends_on` edge leaves this story and it shares no file with a sibling. story:frontier-next-ready-story, the first code story, depends_on this story, and every other code story follows it through the chain, so the nouns are declared before stories build on them, as the epic's Why says: "declared in an ESS domain document before stories build on them".

## Acceptance

`ess specify validate --path ess` exits 0 on a document that declares Case, Run, Workflow, Step and EvidenceProducer with their relations, including the Story → Case relation (one open case per story, resumed by a later run) and the authority grant as the operator decided in decision-blocker:approval-grant-and-case-resume, with no `UNMAPPED:` marker left.

## Out of Scope

Generating Rust from the domain; changing the kernel to match the document; declaring the story-delivery protocol (that is a `protocol/1` document, story:story-delivery-protocol).

## Ambiguities

- Whether this repository wants a specification at all: `AGENTS.md:37` says spec first where one exists and that none exists; this story is the planning skill asking for one, and the operator may archive it to keep the no-spec stance. `inferable`: `AGENTS.md:37`.
- Run resume and approval grant: decided, decision-blocker:approval-grant-and-case-resume (cleared); the document declares what the decision says.

## ## Scope

## Scope

Confirmed by the implementor (wave 01), each line as the unit landed:

- `AGENTS.md` (cited): confirmed; only the sentence saying the repository had no specification
  was replaced.
- `ess/domains/takt.yaml` (inferred → confirmed): created, 778 lines after two adversary passes.
- `ess/ess-inputs.yaml` (inferred → confirmed): created.
- `ess/system.yaml` (inferred → confirmed): created.

Inferred mechanism lines in the body checked against the tree: `run.rs:21-22`, `validator.rs:20-31`,
`run.rs:188-193` (187-193) correct; the Canon citations were corrected to `protocol.yaml:916-923`
and `eval/mod.rs:359-365` after the adversary's pass 1.
