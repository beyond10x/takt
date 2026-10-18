---
format: aep.planning-md/3
id: review-result:adversary-domain-pass-1
kind: review-result
status: active
title: Adversary, takt-domain-model, pass 1
relations:
- reviews: story:takt-domain-model
revision: 1
---
unit: story:takt-domain-model at ee1adfd1c40c18e2b3d93516d1b33e7c225eb02f (branch impl/takt-domain-model, base 1638755a), worktree takt-w1-domain
verdict: CONFIRMED (2 warnings, 7 notes; no blocker, no NEEDS-CHANGE)
cases: executed 1→1 (gate; the implementing state's `0→1`), red 0; scratch probes 10, refused 8 by design, none a defect
origin: introduced 7 / pre-existing 2 / undecided 0
wrote-outside-worktree: 1 directory, 48 files, all under the unit's scratch adversary-1/ (listed in part 6)
needs-coordinator: whether findings 1 and 2 meet the Acceptance as written, given the brief also said "describe what the code does today"; whether the two story citation errors (pre-existing, planning store) go back to the store

## 1. Diff stat

`git --no-pager diff --stat` in the worktree: empty. `git status --short`: empty. No file in the tree was added or changed by this pass; no `ess/scenarios/` file exists (see part 2 for why none could).

## 2. Cases added

None in-tree. An authored conformance scenario cannot be compiled against this document: it declares no command, and `ess verify conform author` refuses any act (verbatim, probe c, scratch):

```
refusal[ESS-AUTHOR-006]: `takt.work/authored/one-open-case-per-story` in .../probe-scenarios/c/one-open-case-per-story.yaml
  `takt.work.OpenCase` is not a command this specification declares
  help: name a command the specification declares; a scenario that invokes anything else is checking a system this model does not describe
0 authored scenario(s) from 1 file(s), 2 refusal(s), suite ess-conformance/34, written to .../probe-scenarios/c/suite.json
EXIT=1
```

Scratch probes run instead, each on a copy (verbatim):

| probe | command | result |
|---|---|---|
| cardinality vocabulary, Story.open_case :377 set to `optional` / `zero_or_one` / `at_most_one` / `zero-or-one` | `ess specify validate --path probe-card` | each refused: ``domains/takt.yaml: unknown variant `optional`, expected `one` or `many` `` |
| schema projection | `ess generate --path probe-schema/spec --kind schema --out probe-schema/out` | exit 0; `required` lists: Case `[id, dir, protocol, intent, authority, evidence_written, state]`, Story `[story_id, status, state]`, Run `[case_id, tree, unit, protocol, inputs, harness, validators, started_at, steps, tokens, state]`, Step `[number, case_id, kind, action, summary, records, state]` — `story`, `open_case`, `workflow`, `producer` are not required |
| compile | `ess specify compile --path ess --format json` | exit 0; relations as declared (6), all `one`/`many` |
| scenario header a/b | `ess verify conform author` | ESS-AUTHOR-001 (wrong header keys), exit 1 |

Order confession (hard rule 3): the gate was run first, as orientation, before any probe existed. No in-tree case was ever added, so no red-before-suite sequence existed to protect; stated so the reader can weigh it.

## 3. The gate

```
$ ess specify validate --path ess
takt v1 — 2 file(s), valid
EXIT=0
$ ess specify validate --path ess --strict-requires
takt v1 — 2 file(s), valid
EXIT=0
$ ess verify conform synthesize --path ess --out .../adversary-1/suite.json
0 scenario(s) (0 authored), 0 refusal(s), written to .../adversary-1/suite.json
EXIT=0
```

No ESS-ENTITY-019 or other warning on stderr. `--format json`: `commands: 0, events: 0, components: 0, unresolved_references: []`. Zero scenarios is what the skill says a domain with no commands, outcomes or invariants yields; not a finding. The suite stays green: nothing in this pass makes it red.

## 4. Findings

All against ee1adfd. `ess/domains/takt.yaml` does not exist at the base, so every row in it is `introduced`; the story file is byte-identical at base, so its rows are `pre-existing`.

| # | file:line | what is wrong | measured | what reaches it | verdict / origin / severity |
|---|---|---|---|---|---|
| 1 | takt.yaml:407 (Run identity `case_id`; relation `case` :482-486 `one`) | Acceptance: Story → Case "resumed by a later run" (decision :49). With Run's identity = CaseId, a second run on one case is a second Run with the same identity: the resume is undeclarable as modelled. Acknowledged in the comment :399-403 and deferred to story:declared-loops. | compile.json: Run.case `cardinality: one`; Run.schema.json identity `case_id` | the Acceptance statement; code today never reaches it (case.rs:44-46 refuses an existing id) | CONFIRMED / introduced / warning |
| 2 | takt.yaml:354 (Story entity) | Decision :47-49 is two writes: `authority.yaml` (typed, Case.authority :320-321) and "records an `approval` evidence on the story in the planning store" — prose only, at :314. Story has `status` and `open_case`; the Outcome asks that these be checkable "instead of reading them out of story bodies". | compile.json: Story fields `status`, `open_case` only | the Acceptance's "authority grant as the operator decided" | CONFIRMED / introduced / warning |
| 3 | takt.yaml:575 | Step.action comment: the judgement action "must be among those offered or the step is refused". run.rs:262-278 accepts the gate action, which is never in `offered` (approval-required ≠ admissible; run.rs:215-219, 229-233). The cited harness.rs:21-22 ("nothing else is accepted") is itself contradicted by run.rs:262 (that code comment is pre-existing). The entity header :552 does say it; the field comment does not. | read, not run | every run with `--gate` (default `repository.merge`, main.rs:67) | CONFIRMED / introduced / note |
| 4 | takt.yaml:296 | Case.protocol typed `takt.work.ProtocolId` = `<name>/<major>` (:35-37) while the entity is "the Canon case on disk" (:279) whose `case.yaml` `protocol` key is the name alone (case.rs:69; canon Case.protocol is `canon.protocol.ProtocolId`, protocol.yaml:873-874). The document states both facts and types the field by the in-memory one. | Case.schema.json requires `protocol` (a String newtype, so no schema failure) | a reader checking `case.yaml` against the Case entity | CONFIRMED / introduced / note |
| 5 | takt.yaml:463 | Run.started_at: `Timestamp`, read from run.rs:91/120, which hold a monotonic `std::time::Instant` (elapsed only); no wall-clock start is stored; the case id's instant is a separate `now()` call (run.rs:111). | read | nothing in code produces the value as typed | CONFIRMED / introduced / note |
| 6 | takt.yaml:564 | Step identity `number` is unique only within a run (:499); under `owns via case_id` the key is at best (case_id, number), and after the decided resume not even that (steps restart at 0, run.rs:118). ESS has struct identities (later-formats.md:301). | Step.schema.json identity `number` | nothing today; the decided resume | CONFIRMED / introduced / note |
| 7 | takt.yaml:219 | ProtocolBlock declares `id, inputs`; the only source, docs/design/takt.md:55, lists `id, revision, inputs`. "with its major in the id" (:214-215) is a fold no source states. | read | the M3 loader (design :74) | CONFIRMED / introduced / note |
| 8 | .engineering/planning/story/takt-domain-model.md:39 | cites canon 0.6.0 protocol.yaml:644-651 for `canon.protocol.Case` relation `protocol`; those lines are a `canon.protocol.Principal` field and `case_revision` of another struct. The relation is at :916-923 (the document's :266 claim is right, citing :861). | `git show 0.6.0:ess/domains/protocol.yaml` | anyone following the story's citation | CONFIRMED / pre-existing / note |
| 9 | .engineering/planning/story/takt-domain-model.md:43 | cites canon eval/mod.rs:319-326 for the per-evaluation authority list; those lines are `Refusal::new`. `Supplied.authority` is at :359-365, which the document cites correctly (:318). | `git show 0.6.0:crates/canon/src/eval/mod.rs` | same | CONFIRMED / pre-existing / note |

Named fixes (not applied): 1 — either a Run identity of its own with `Run.case` kept `one` and a comment that Case→Run is many once resume lands, or an explicit statement in the entity that the acceptance's resume is deferred; 2 — a typed `Story.approvals` (or equivalent) or a sentence in the Story entity saying the planning-store record is aep's and out of this document; 3 — ":575 … among those offered, or the run's gate action, or the step is refused"; 4 — type the field by what `case.yaml` holds or rename the entity's claim; 5 — `started_at` as the instant the case id is minted from, or drop it; 8/9 — store edits, coordinator's.

## 5. Attacked and not broken

- All in-repository `# read from:` citations (run.rs, case.rs, harness.rs, validator.rs, evidence.rs, protocol.rs, main.rs, docs/design/takt.md, the decision file :46-52, the story file :42): each line says what is claimed, except finding 3.
- All cross-repository citations at the pinned releases: canon 0.6.0 protocol.yaml lines 32, 37, 42, 57, 62, 67, 241, 293, 526, 533, 538, 571, 682, 861 (+ relation 916-923); model/authority.rs:24, :34; eval/authority.rs:7-21 (duplicate-identifier refusal, as :311 says); eval/mod.rs:359-365. aep commit 6d7a44d3 carries tag 0.68.0; story.yaml:14-22 and :43-49 say what :51-58 claims; the six StoryStatus variants match.
- `takt.work.Stop`: all 8 variants and payload types match run.rs:41-58; the four stop sites (:174-183, :220-227, :262-297) match the comment.
- Optional `via` under `cardinality: one` (Case.story, Story.open_case, Run.workflow, Step.producer): ESS's vocabulary is `one | many` only, and the projection keeps the four fields optional, so nothing is told they are required; "at most one" is as declarable as ESS allows.
- The five demanded nouns are present under the story's names; `Unit`, `Story`, `Grant`, `CapabilityId` present; `grep UNMAPPED ess/` finds nothing.
- `Run.gate`, `Run.protocol`, `Run.inputs`, `Run.validators`, `Case.authority` (Map keyed by capability; canon refuses a duplicate), `Case.evidence_written`, `Step.committed`, `Step.records`, `EvidenceProducer.{action,kind,subject}`: match the code.
- Confidentiality grep of the diff (home paths, host, session or decision ids, persons): nothing. AGENTS.md:37 replaced; no other "no specification" sentence in README or docs.
- `ess-inputs.yaml` `requires: ess 0.60` under `--strict-requires`: valid.

## 6. Paths written outside the worktree

All under the unit's scratch adversary-1/ (created by this pass):
- suite.json; compile.json
- probe-card/{system.yaml, ess-inputs.yaml, domains/takt.yaml} (mutated copy)
- probe-schema/spec/{system.yaml, ess-inputs.yaml, domains/takt.yaml} (copy); probe-schema/out/ (ess generate: schema/entities 6 files, schema/types 25 files, .ess-output/state.json)
- probe-scenarios/{a,b,c}/one-open-case-per-story.yaml and probe-scenarios/{a,b,c}/suite.json

Nothing under /tmp. Session lease `takt-w1-domain-adv1` taken and released (`session-end` exit 0).

## 7. Findings block

```findings
- file: ess/domains/takt.yaml
  line: 407
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: Run's identity is the CaseId with Run.case cardinality one, so the acceptance's "resumed by a later run" cannot be declared; the comment at 399-403 defers it rather than declaring it.
- file: ess/domains/takt.yaml
  line: 354
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: The decision's second write, the approval evidence recorded on the story in the planning store, has no typed home on Story and lives only in the Case.authority comment at 314.
- file: ess/domains/takt.yaml
  line: 575
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: Step.action's comment says an action not among those offered is refused, but run.rs:262-278 accepts the gate action, which is never in the offered set.
- file: ess/domains/takt.yaml
  line: 296
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: Case.protocol is typed as the name/major id while the on-disk case.yaml protocol key the entity describes holds the name alone (case.rs:69).
- file: ess/domains/takt.yaml
  line: 463
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: Run.started_at is typed Timestamp but the cited run.rs:91 and :120 hold a monotonic Instant and no wall-clock start exists.
- file: ess/domains/takt.yaml
  line: 564
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: Step's identity number is unique only within a run, and after the decided resume not even within a case; a struct identity would carry the real key.
- file: ess/domains/takt.yaml
  line: 219
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: ProtocolBlock folds the design's separate revision field (docs/design/takt.md:55) into id with no source for the fold.
- file: .engineering/planning/story/takt-domain-model.md
  line: 39
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: The story cites canon protocol.yaml:644-651 for Case's protocol relation; the relation is at 916-923 and 644-651 is another struct's Principal field.
- file: .engineering/planning/story/takt-domain-model.md
  line: 43
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: The story cites canon eval/mod.rs:319-326 for the per-evaluation authority list; those lines are Refusal::new and Supplied.authority is at 359-365.
```
