# Takt: composable agentic work

Status: initial design, 2026-10-18. Milestone M1 is done (`docs/runs/2026-10-18-m1-scratch.md`); nothing below M1 is built.

## 1. The target

Safe agentic coding, applied. A YAML description of workflows made of protocols (a software change
is one, from engineering protocols) and building blocks: inner loops, outer loops, validation
loops. A harness such as Claude Code understands the protocols and the blocks and drives a workflow
through the model, in any code repository, ad hoc or from the repository's default. Every step is
validated by a hard gate: Canon evaluates the case over evidence, and AEP is the record.

The result is one binary. Point it at a repository and say "work on this": it pulls the next unit
from the planning store (or takes an intent), opens a case, and runs one iteration, three, or three
hours, by the stop criteria given. Everything is pluggable and composable.

The name: takt time is the pace at which units of work are finished. One run is one takt.

## 2. Foundations

| foundation | what it gives | release |
|---|---|---|
| Canon | `protocol/1`; `evaluate` is a pure function from protocol, case snapshot, evidence records, authority and instant to claims (TRUE, FALSE, UNKNOWN), obligations, admissible actions and legitimate outcomes, with an explanation; evidence is bound to the revision it was observed on | `b10x-canon` 0.6.0 |
| Engineering protocols | `software.change/1` and the other engineering protocols as Canon documents, validated by Canon; assertion providers and the gates command line | `b10x-canon-engineering` 0.9.0 |
| AEP | the planning store: stories, scope, waves derived from scope, evidence on a story | `aep` 0.74.0 |
| a harness | the judgement actor: Claude Code first (`claude -p`), another behind the same trait later | |

Two rules carried over from the thesis these foundations serve: "Agents choose how to do the work.
The organization defines what must be true before the work counts", and a run completing or a pull
request merging is not completion; completion is earned when the protocol's conditions hold.

## 3. Shape

```
takt run --repo <path> [--intent "<text>"] [--workflow <file>]
         [--iterations N | --hours H | --tokens T | --until <outcome>]

  unit     := intent | frontier(repo)              AEP: ready stories and waves; no model
  case     := open(protocol, unit, worktree)       Canon case files under .engineering/cases/<id>/
  loop     := evaluate → frontier → step → effect → observation → validator → evidence → evaluate
                judgement step: the harness performs ONE admissible action in the worktree and reports
                deterministic step: a command (a gate, a pull request, a CI read, a merge on approval)
             until an outcome is legitimate | an approval is needed | the stop criteria | nothing admissible
  record   := run events (JSONL), every Canon decision, AEP evidence on the unit
```

The hard rule: the model never produces evidence. A validator, a program, turns an observation
(a test run's exit status, a commit, a merged pull request) into a `canon-evidence/1` record bound
to the revision it observed. What the harness says is a trace.

### Building blocks of `workflow/1`

| block | fields | example |
|---|---|---|
| `protocol` | id, revision, inputs | `software.change/1`, risk `standard`, change_kind `bugfix` |
| `step` deterministic | action, argv | `tests.run` → `cargo test --quiet` |
| `step` judgement | actions (a subset of the protocol's), brief | implement among `tests.write`, `implementation.edit`, `repository.inspect` |
| `loop` | steps, until (a claim, an outcome or a budget) | inner: until `tests.pass`; validation: until the gate is green; outer: the next story until none is ready or the budget is spent |
| `gate` | a `canon-expr/1` assertion and its provider | `tests.unit.failed == 0`; later `pr.merged`, `ci.green` |
| `approval` | capability, approver | `repository.merge` by the operator |

### Seams

Traits in the library, one adapter each at first: `Harness` (the judgement actor), `Frontier` (the
planning store), `Validator` (an evidence producer), `Forge` (pull request, merge, release through
the repository's bot route), `Confinement` (none first; Substrate later).

## 4. Milestones

| m | deliverable | proof |
|---|---|---|
| M1 one step validated | the kernel (case files, Canon evaluation), `takt run --repo R --intent "…" --iterations 1` on `software.change/1`, the Claude Code adapter (one action per step in a worktree), a `tests.run` validator, the decision document printed | a scratch crate with one failing test: `tests.pass` TRUE, `repository.merge` approval-required, 0 evidence from the model |
| M2 a story end to end | the AEP frontier (ready rule as code, `artifact waves`); a protocol for a story with claims `tests.pass`, `gate.green`, `pr.opened`, `ci.green`, `pr.merged` and outcome `accepted`; `approval` for the merge; `--hours`, `--tokens`; AEP evidence written | one ready story of a small repository done with no manual step but the merge approval |
| M3 workflows | the `workflow/1` schema and loader; inner, validation and outer loops as declared blocks; a repository default in `.engineering/workflow.yaml` | the same story from a YAML file, and a three-iteration outer loop |
| M4 pluggable | the five traits; a second harness adapter proves the seam; validators from the engineering assertion providers; Substrate optional | the M2 story runs under both adapters with the same decision documents |
| M5 prove | three repositories, each a daily `run --hours 2` for a week; units accepted, tokens, human interventions, defects escaped | the numbers |

## 5. Not in this design

A hosted service, a console, a database, cross-repository scheduling, and any runtime of its own
for model turns: the harness is an adapter, the loop is in this binary, the truth is Canon's.

Continuous work and orchestration inside one repository, planning as a unit, every loop a case in
one tree, one deterministic dispatcher, and cross-repository scheduling as its last step, are
designed in the planning-store artifact `design:continuous-orchestration`
(`.engineering/planning/design/continuous-orchestration.md`).

## 6. Self-improvement

takt is improved by takt: the improvement process is itself modelled as protocols, and the same
loop improves the loop. The design is the planning-store artifact
`design:recursive-self-improvement` (`.engineering/planning/design/recursive-self-improvement.md`),
carried by `epic:recursive-self-improvement`.

| level | protocol | one case is |
|---|---|---|
| L0 work | `protocols/story-delivery/1.yaml` | one story of a repository |
| L1 improve takt | `protocols/self-improvement/1.yaml` | one finding reproduced on the promoted baseline, fixed, verified by another run, merged under the authority its surface needs, scored without regression, promoted |
| L2 improve the loop | `protocols/loop-improvement/1.yaml` | one loop metric below target, an honest change to the loop merged by the owner, the metric improved |

`crates/takt/tests/improvement_protocols.rs` drives cases on both improvement protocols to their
outcomes. The validators for their actions are stories of that epic; until they land, the agents
perform the actions and record the evidence.
