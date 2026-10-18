---
format: aep.planning-md/3
id: story:gate-validator
kind: story
status: implemented
title: A gate validator turns an engineering-gates run into gate_result evidence
summary: gate.green from a typed engineering-gates/1 document, pass or fail, nothing on unknown
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:frontier-next-ready-story
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/gate_cli.rs
- confidence: cited
  path: crates/takt/Cargo.toml
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: crates/takt/tests/fixtures/gate-only/1.yaml
- confidence: inferred
  path: crates/takt/tests/gate_validator.rs
- confidence: cited
  path: crates/takt/tests/story_delivery.rs
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T04:27:09Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-18T04:27:09Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-18T05:45:12Z", actor: "human:timo", revision: 13, decided_on: {"recorded":{"test_result":1,"review_outcome":4}}}
---
# Story: gate-validator

## Outcome

A claim `gate.green` is true only when the repository's own `engineering-gates/1` document evaluated true at the current implementation revision, and the evidence record says which run said so; no model and no exit code of an arbitrary command stands in for it.

## Context

`docs/design/takt.md` § 3 Building blocks: `gate | a canon-expr/1 assertion and its provider | tests.unit.failed == 0; later pr.merged, ci.green`; § 4 M4: "validators from the engineering assertion providers". Today the only validator is `TestsRun` (`crates/takt/src/validator.rs:33-110`), which runs one command and maps its exit status to pass or fail. The engineering protocols ship typed gates: a `.engineering/gates.yaml` in `engineering-gates/1` with `canon-expr/1` assertions (b10x-canon-engineering 0.9.0, `fixtures/assertions/gates.yaml`), collected and evaluated by `gates run` and replayed by `gates evaluate` (`crates/canon-engineering-assertions/src/cli.rs:43-61`), with exit 0 true, 1 false, 2 invalid, 3 unknown (`website/docs/guides/assertions.md`), and providers such as `tests.run`, `process.succeeded`, `fs.file.exists` (`crates/assertion-providers/README.md`).

This story adds a `gate.run` validator: it evaluates the gates document (through the library `b10x-canon-engineering-assertions`, or the `canon-engineering` command as a child process; both are released), writes one `gate_result` record about `implementation` at the current revision with result `pass` on true and `fail` on false, names itself as `producer` (`crates/takt/src/evidence.rs:16`) and the retained evidence path as its reference, and writes nothing on unknown or invalid, because UNKNOWN is not FALSE. A `gate` block inside a `workflow/1` file (story:workflow-schema-and-loader) is an assertion with its provider in the same `canon-expr/1` form, and this validator evaluates it the same way as `.engineering/gates.yaml`: the block is given to the validator as an assertion source beside the gates file, collected and evaluated by the same route, and the record is the same `gate_result`; the loader passes the block through and evaluates nothing itself. The loop already runs a validator whose action is admissible and whose evidence is missing at the current revision (`crates/takt/src/run.rs:159-187`), so no loop change is needed beyond registration.

## Domain relations

- Validator → Action, EvidenceKind, Subject: exactly one each. Inferred from `crates/takt/src/validator.rs:20-31`; the loop picks the first due validator, `crates/takt/src/run.rs:161-167`.
- EvidenceRecord → artifact revision: one subject at one revision. Inferred from b10x-canon 0.6.0 `crates/canon/src/model/evidence.rs:30-35`.

## Scope

- `crates/takt/src/validator.rs` (`GateRun`, over the gates file or a `gate` block's assertion)
- `crates/takt/Cargo.toml` and `Cargo.toml` (the assertions crate, if the library route is taken)
- `crates/takt-cli/src/main.rs` (`--gates <path>`, default `.engineering/gates.yaml`; registered when the file exists)
- new: `crates/takt/tests/gate_validator.rs` with a minimal `protocol/1` fixture declaring `gate_result` and `gate.green`, and a scratch repository with a gates file

## Ordering

- depends_on story:frontier-next-ready-story: both edit `crates/takt-cli/src/main.rs`.

## Acceptance

On a scratch repository whose `.engineering/gates.yaml` holds one assertion, a run with `--harness none` on a protocol declaring `gate.green` writes exactly one `gate_result` record about `implementation` at HEAD whose result is `pass` when the assertion is true and `fail` when it is false, the next decision document shows `gate.green` true or false accordingly, and an assertion that evaluates unknown writes no record.

## Out of Scope

The loader that reads a `gate` block out of a `workflow/1` file (story:workflow-schema-and-loader); network and Kubernetes providers; replacing `TestsRun` with the `tests.run` provider.

## Ambiguities

- Library versus child process: `inferable`, both exist in b10x-canon-engineering 0.9.0 (`crates/canon-engineering-assertions/src/lib.rs`, `src/cli.rs`); the implementer chooses, and the record is the same either way.

## ## Scope

## Scope

Confirmed by the implementor (wave 03), each line as the unit landed:

- `crates/takt/src/validator.rs` (cited): confirmed; `GateRun` over the gates document through the
  assertions library, `Observation.artifact`, the project file with its default.
- `crates/takt/Cargo.toml` and `Cargo.toml` (cited): confirmed; `b10x-canon-engineering-assertions`
  and `b10x-canon-expr` as workspace dependencies; `Cargo.lock` gained their two entries.
- `crates/takt-cli/src/main.rs` (cited): confirmed; `--gates` and `--project` optional, resolved
  under the repository or refused before any case is opened, the validator registered when the
  document is a file.
- `crates/takt/tests/gate_validator.rs` and `crates/takt/tests/fixtures/gate-only/1.yaml`
  (inferred → confirmed): created; 16 cases, 11 of them the adversary's.
- `crates/takt/src/run.rs` (added in correction 1): the due rule for a validator that observed
  without a record, the retained artifact written before the records, idle only when nothing
  is due.
- `crates/takt-cli/tests/gate_cli.rs` (added by the adversary, adopted by the unit): the binary
  end to end on scratch repositories; 8 cases.
- `crates/takt/tests/story_delivery.rs` (added in correction 1): one field in a literal.
