---
format: aep.planning-md/3
id: review-result:adversary-domain-pass-2
kind: review-result
status: active
title: Adversary, takt-domain-model, pass 2
relations:
- reviews: story:takt-domain-model
revision: 1
---
unit: story:takt-domain-model pass 2, at 23e05b9 (branch impl/takt-domain-model, base 1638755a), worktree takt-w1-domain
verdict: CONFIRMED (1 warning, 3 notes; no blocker, no NEEDS-CHANGE)
cases: executed 1→1 (gate; the implementing state's count), red 0; scratch probes 7, none red against the document
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, 70 files, all under the unit's scratch adversary-2/ (part 6)
needs-coordinator: whether finding 1 (Approval's `--source`/`--at` wording vs story:merge-under-authority:40) is reconciled in this document or in that store artifact; the store is yours

## 1. Diff stat

`git --no-pager diff --stat` in the worktree: empty. `git status --short`: empty. No tree file added or changed; no `ess/scenarios/` file exists (part 2 says why).

## 2. Cases added

None in-tree. The document still declares no command (`validate --format json`: `commands: 0`), so an authored scenario is structurally refused. Re-probed on a copy of the pass-2 document with pass 1's scenario file, verbatim:

```
$ ess verify conform author --path spec --scenarios one-open-case-per-story.yaml --out suite.json
refusal[ESS-AUTHOR-006]: `takt.work/authored/one-open-case-per-story` in one-open-case-per-story.yaml
  `takt.work.OpenCase` is not a command this specification declares
  help: name a command the specification declares; a scenario that invokes anything else is checking a system this model does not describe
0 authored scenario(s) from 1 file(s), 2 refusal(s), suite ess-conformance/34, written to suite.json
EXIT=1
```

Scratch probes, each on a copy under adversary-2/:

| probe | command | result |
|---|---|---|
| schema projection (the coordinator's question) | `ess generate --path probe-schema/spec --kind schema --out probe-schema/out` | exit 0, 41 artifacts. Step identity `key` → `$ref takt.work.StepKey` (struct, required `[run_id, number]`); `ProtocolSource` → `oneOf` tagged `kind` with `builtin: $ref ProtocolId` and `file: string`. Neither shape refused |
| Rust types | `ess generate types --path spec --target rust --all-types --package takt-probe --out out-rust` | exit 0, 35 model types |
| docs | `ess generate --path spec --kind docs --out out-docs` | exit 0, 5 artifacts |
| compile | `ess specify compile --path ess --format json` | exit 0; Run identity `run_id`, Step identity `key: takt.work.StepKey`, Run.steps `owns … via run_id` |
| mutant: `Step.run_id` field removed, `via: run_id` kept (tests the :664-665 claim that `via` needs a field) | `ess specify validate --path probe-via/spec` | refused, so the claim holds: `error[ESS-ENTITY-005]: relation `steps` of `takt.work.Run` is carried by `run_id`, which `takt.work.Step` does not declare … fields of `takt.work.Step`: key, kind, action, producer, summary, records, tokens, committed --> domains/takt.yaml:472:5` EXIT=1 |

Order confession (hard rule 3): the three gate commands ran first, as the coordinator's named gate, before any probe existed. No in-tree case was ever possible, so no red-before-suite sequence existed to protect.

## 3. The gate

```
$ ess specify validate --path ess
takt v1 — 2 file(s), valid
EXIT=0
$ ess specify validate --path ess --strict-requires
takt v1 — 2 file(s), valid
EXIT=0
$ ess verify conform synthesize --path ess --out .../adversary-2/suite.json
0 scenario(s) (0 authored), 0 refusal(s), written to .../adversary-2/suite.json
EXIT=0
```

No ESS-ENTITY-019 or other stderr warning. The suite stays green; nothing in this pass makes it red.

## 4. Findings

All against 23e05b9. `ess/domains/takt.yaml` does not exist at the base, so every row is `introduced`.

| # | file:line | what is wrong | measured | what reaches it | verdict / origin / severity |
|---|---|---|---|---|---|
| 1 | ess/domains/takt.yaml:145 (Approval comment; repeated in the `approver` summary :156) | "with the approver as the record's `--source` and the instant as its `--at`" is cited to decision :47-49, which names neither flag; story:merge-under-authority (`.engineering/planning/story/merge-under-authority.md:40`) spells the same write `--source takt --ref <case>/authority.yaml` and its Checks expect a record "whose source names takt", with the approver carried by `--by <approver>`. The document invents a mechanism no source states and the sibling story contradicts it. | read; aep 0.74.0 `aep plan artifact evidence --help` has both flags (`--source`: "a person's name" is one allowed value), so the flags exist and the assignment is the invention | whoever implements `takt approve` from this type (story:merge-under-authority, draft); nothing in code today | CONFIRMED / introduced / warning |
| 2 | ess/domains/takt.yaml:149 (Approval struct) | No field for the case the grant was written into; merge-under-authority.md:40 writes `--ref <case>/authority.yaml` on the record, and the typed record has no home for it, so an approval on a story cannot be matched to its case from this type. | compile.json: Approval fields `capability, approver, granted_at` | the `--ref` the sibling story writes; no story with a second case was shown | CONFIRMED / introduced / note |
| 3 | ess/domains/takt.yaml:554 (Run.started_at; comment :546-548) | Defined as the instant of `crate::now()` at run.rs:111, which sits inside `unwrap_or_else` and never runs when `--case-id` is given (main.rs:63-65); the projection makes the field required (`Run.schema.json` `required` lists `started_at`, `format: date-time`), so on that path it has no source. | Run.schema.json `required`; run.rs:108-111 read | `takt run --case-id <id>` | CONFIRMED / introduced / note |
| 4 | ess/domains/takt.yaml:643-646 (Step entity comment) | The refusal list reads as exhaustive ("an action neither offered nor the gate, a `repository.inspect` …, or a `tests.write` …, is refused") and omits the gate action requested with a change: run.rs:263-274 discards the change and stops `StepRefused("<gate> requested with changes")`. Same omission at ee1adfd:550-552; the field comment :674-677 is loose enough to cover it, the entity comment is not. | run.rs:262-278 read | any run with `--gate` (default `repository.merge`, main.rs:67) whose harness names the gate after editing a file | CONFIRMED / introduced / note |

Named fixes (not applied): 1 — cite nothing for the flags, or say "the record's source and instant as `aep plan artifact evidence` records them; which value names the approver is story:merge-under-authority's"; 2 — add `case: takt.work.CaseId` to Approval, or say the case is the record's `--ref`; 3 — `Optional<Timestamp>`, or define it as the instant `now()` would read at open and say the kernel takes it only on the generated-id path; 4 — add "the gate action requested with a change" to the refusal list at :644.

## 5. Attacked and not broken

- Answer 1 (RunId): :38-41 and :483-486 cite run.rs:78-93 (no id field), :108-112, main.rs:171-176 (the stop line prints the case dir), decision :49-51 — all true. Run identity `run_id`, `Run.case` kept `one` via `case_id`; Case :335-337 names story:merge-under-authority, whose Scope owns `takt run --case <id>` and "a run resumed on an open case". The acceptance's "resumed by a later run" is now declarable (two Runs may reference one Case) and is stated at :335-337 and :571.
- Answer 2 (Story.approvals): `List<takt.work.Approval>` typed; :447 decision :47-49 true for "writes authority.yaml and records an approval evidence on the story" — only the flag assignment (finding 1) is uncited.
- Answer 3 (Step.action): run.rs:229-233, :262-278, :279-280, harness.rs:21-25, :36-38 all say what is claimed. "never in the offered set" holds by construction: decision.rs:14 and :60-76 give each action one status, `admissible` xor `approval-required`.
- Answer 4 (ProtocolName / Case.protocol / ProtocolSource): protocol.rs:13-14, case.rs:69, :51-52 true; canon 0.6.0 protocol.yaml:32 is `canon.protocol.ProtocolId` and Case.protocol :873-874 carries it, the name alone (protocol.rs:58 reads `protocol.id` = name), so moving the Canon reference to ProtocolName is right. ProtocolSource: run.rs:19-20, :103-107 true; projected and typed without refusal (part 2).
- Answer 5 (started_at): run.rs:111, lib.rs:24-32, :91, :120, :131-135 all true; only the `--case-id` path is uncovered (finding 3).
- Answer 6 (StepKey): run.rs:118, :195, :228 true; the struct identity passes validate, strict, synthesize, schema, docs and Rust types; the duplicate `run_id` field's justification (:664-665) is proven by the ESS-ENTITY-005 mutant (part 2); `Step.run_id: RunId` matches Run's identity type as `owns … via` requires.
- Answer 7 (ProtocolBlock): design :55 lists `id, revision, inputs`; run.rs:18-24 and protocol.rs:13-16 true; `revision: Integer` is its own field.
- Regression: every citation the diff removed was moved, not lost (case.rs:69 → ProtocolName; run.rs:82 → Run.case_id; harness.rs:21-22 → :21-25); every other `# read from:` at 23e05b9 matches the line it names; no `UNMAPPED:` in the file; the diff ee1adfd..23e05b9 names no private id, host or person.
- Pass-1 findings 8 and 9 (story citations) are the store's; not re-attacked here.

## 6. Paths written outside the worktree

All under the unit's scratch `adversary-2/` (created by this pass), 70 files:
- `suite.json`, `compile.json`, `compile.stderr`
- `probe-schema/spec/` (copy, 3 files) and `probe-schema/out/` (41 schema files + `.ess-output/state.json`)
- `probe-via/spec/` (mutated copy, 3 files)
- `probe-types/spec/` (3), `probe-types/out-rust/` (`types.rs`, `source.schema.json`, `types-report.json`, …), `probe-types/out-docs/` (5)
- `probe-scenario/spec/` (3), `probe-scenario/one-open-case-per-story.yaml` (copied from adversary-1), `probe-scenario/suite.json`

Nothing under /tmp. Lease `takt-w1-domain-adv2` taken and released (`session-end` exit 0).

## 7. Findings block

```findings
- file: ess/domains/takt.yaml
  line: 145
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "The Approval comment assigns the approver to the evidence record's --source and the instant to --at, which the cited decision never says and story:merge-under-authority:40 contradicts with --source takt --ref <case>/authority.yaml and --by <approver>."
- file: ess/domains/takt.yaml
  line: 149
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Approval carries no reference to the case its grant was written into, while the sibling story writes --ref <case>/authority.yaml on the record, so the typed record cannot be matched to its case."
- file: ess/domains/takt.yaml
  line: 554
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Run.started_at is defined as the instant run.rs:111 mints the case id from, which never runs when --case-id is given, yet the projection makes the field required."
- file: ess/domains/takt.yaml
  line: 643
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The Step entity comment's refusal list omits the gate action requested with a change, which run.rs:263-274 refuses and discards."
```
