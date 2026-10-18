---
format: aep.planning-md/3
id: story:workflow-schema-and-loader
kind: story
status: draft
title: Load a workflow/1 file and run the intent case from it
summary: The schema, the loader and the repository default; the same decisions as the flag-driven run
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:run-record-and-story-evidence
- depends_on: story:gate-validator
- serves: vision:safe-agentic-coding
- depends_on: story:story-done-end-to-end
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/workflow.rs
- confidence: inferred
  path: crates/takt/tests/workflow.rs
- confidence: inferred
  path: docs/workflow-1.md
revision: 9
---
# Story: workflow-schema-and-loader

## Outcome

An operator describes a run in one YAML file, the protocol and its inputs, each deterministic step with its argv, the judgement step with its actions and brief, a gate and an approval, and `takt run --workflow <file>` produces the same decisions as the same run described by flags; a repository carries its default in `.engineering/workflow.yaml`.

## Context

`docs/design/takt.md` § 3 Building blocks of `workflow/1`: `protocol` (id, revision, inputs), `step` deterministic (action, argv), `step` judgement (actions, a subset of the protocol's, and a brief), `loop` (steps, until), `gate` (a `canon-expr/1` assertion and its provider), `approval` (capability, approver); § 4 M3: "the `workflow/1` schema and loader; ... a repository default in `.engineering/workflow.yaml`". Today a run is configured by flags only (`crates/takt-cli/src/main.rs:26-60`) and the validator list is fixed in the binary (`main.rs:142`).

This story: the `workflow/1` document (`format: workflow/1`), a loader that refuses what it cannot run (an unknown block kind, a judgement step naming an action the protocol does not declare, checked against `LoadedProtocol.actions`, `crates/takt/src/protocol.rs:25-26`), and the mapping to what exists: `protocol` → `LoadedProtocol` and the case inputs (`crates/takt/src/case.rs:70-92`); a deterministic `step` → a validator running that argv for that action, in the shape of `TestsRun` (`crates/takt/src/validator.rs:33-49`); the judgement `step` → the actions offered to the harness and the brief in its prompt; `gate` → the `gate.run` validator of story:gate-validator, which evaluates the block's `canon-expr/1` assertion with its provider exactly as it evaluates `.engineering/gates.yaml` and writes the same `gate_result` record, so the loader hands the block over and evaluates nothing itself; `approval` → story:merge-under-authority's gate and grant record: the block's capability becomes the run's gate (what `--gate` sets today, `crates/takt-cli/src/main.rs:66-68`) and its approver is recorded with the grant in the case's `authority.yaml` and the story's `approval` evidence, so the loader hands the block over and enforces nothing itself; `loop` blocks are parsed and validated here and executed in story:declared-loops. A loop's steps may contain a loop, because the outer loop of § 3 holds the inner and validation loops. Precedence: `--workflow <file>`, else `.engineering/workflow.yaml` when present, else flags; `--protocol` and `--test-cmd` beside a workflow are refused rather than merged.

## Domain relations

- Case → Protocol: exactly one, so a workflow names one protocol per case it opens. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:644-651`, entity `canon.protocol.Case`, relation `protocol`.
- Validator → Action: exactly one; a deterministic step is one validator. Inferred from `crates/takt/src/validator.rs:20-31`.
- Harness step → Action: exactly one of the offered. Inferred from `crates/takt/src/run.rs:188-193`.

## Scope

- new: `crates/takt/src/workflow.rs` (types, loader, validation)
- `crates/takt/src/lib.rs` (the `workflow` module registered)
- `crates/takt/src/run.rs` (validators and the offered actions built from the workflow)
- `crates/takt/src/harness.rs` (the brief from the judgement step)
- `crates/takt-cli/src/main.rs` (`--workflow`, the repository default, the refusals)
- new: `docs/workflow-1.md` (the schema, one example per block)
- new: `crates/takt/tests/workflow.rs` and `crates/takt/tests/fixtures/workflow.yaml`

## Ordering

- depends_on story:run-record-and-story-evidence: both edit `crates/takt/src/run.rs`, `crates/takt-cli/src/main.rs` and `crates/takt/src/lib.rs`.
- depends_on story:gate-validator: both edit `crates/takt-cli/src/main.rs`; the `gate` block is evaluated by the validator that story adds.
- depends_on story:story-done-end-to-end: both edit `crates/takt/src/run.rs` and `crates/takt-cli/src/main.rs`; the joined path that story fixes in those files is what the loader configures from a file.

## Acceptance

On the scratch crate used for M1, `takt run --repo R --intent I --iterations 1 --test-cmd C` and `takt run --repo R --intent I --iterations 1 --workflow W`, with W declaring `software.change/1` and a `tests.run` step with argv C, produce decision documents equal in every claim value, action status and outcome status.

## Checks

- A workflow naming an action the protocol does not declare is refused before any case is opened.
- A workflow with a `gate` block runs the `gate.run` validator of story:gate-validator on that block's assertion, and the decision shows `gate.green` from the `gate_result` record it wrote.
- `--protocol` or `--test-cmd` given beside `--workflow` is refused before any case is opened.

## Out of Scope

Running loops (story:declared-loops); the story run from a `workflow/1` file, the M3 proof, which is story:declared-loops' check on the story story:story-done-end-to-end finished; `--until`; evaluating a `gate` block's assertion (story:gate-validator); reading an `approval` block's capability into the gate and recording its approver with the grant (story:merge-under-authority); a workflow registry or includes; more than one protocol in one file.

## Ambiguities

- Whether `loop.steps` may hold a loop: `inferable`, `docs/design/takt.md` § 3 loop row makes the outer loop contain the inner and validation loops.
- Flags beside a file: `inferable` as recorded here, refused, so a run has one source of truth; the operator can change this to override semantics in one place.

## Constraint from the store guard

Since story:run-record-and-story-evidence (granted at f6056dd, wave 09), a judgement step that changes any path under `.engineering/` other than `cases/` is refused and put back: the planning store, the evidence directory, `project.yaml`, `gates.yaml`, `assertions-project.yaml` and any new file such as `.engineering/workflow.yaml`. Found by the reviewer's verdict on that unit.

This story as drafted conflicts with it in two places: its `planner` role writes `.engineering/planning/story/**` through `implementation.edit`, and a repository's default workflow is `.engineering/workflow.yaml`, which no takt step could then add or change. Before it is scheduled: a planner's store writes become deterministic actions (an `aep plan artifact` verb run by takt, its output a validator record), never a judgement step's file edit; and the workflow file is added or changed by a person or the developer role, as gates and project files are.
