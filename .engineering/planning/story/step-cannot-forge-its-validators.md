---
format: aep.planning-md/3
id: story:step-cannot-forge-its-validators
kind: story
status: active
title: A judgement step cannot forge the validators that judge it
summary: tests.run and ci.read run their inputs from the base or refuse; a judge-surface edit is refused or flagged
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/step_cannot_forge_its_validators.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 7}
---
# Story: step-cannot-forge-its-validators

## Outcome

A judgement step cannot change what decides it. A step that edits an input a validator runs (the test command's script, `.cargo/**`, `build.rs`, `rust-toolchain*`, the CI workflow) earns no `tests.pass` or `ci.green` from that edit: the validator runs those inputs at the base revision, or refuses to record while the case's branch changed them. A step that touches the repository's judge surface is refused, or named in the pull-request body and in `takt approve`.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4). Severity high.

- Verified by the review: `implementation.edit` can commit `.cargo/config.toml` with `runner = "true"` (or a `rustc-wrapper`), and `tests.run` then records `pass` on a test that fails. The refusal list a step's change passes through, `crates/takt/src/run.rs:1036-1049`, refuses an action that was not offered, a `repository.inspect` that changed files and a `tests.write` outside test paths; it has no rule on which paths `implementation.edit` may change.
- Inferred from the code: `tests.run` runs the test command from the tree (`crates/takt/src/validator.rs:66-68`, "The command is code from the tree"). An edited `./check.sh` that ends in `exit 0` earns `tests.pass` the same way.
- Inferred from the code: an edit under `.github/workflows/**` decides what `ci.read` observes, so the same step can earn `ci.green`.
- The gate already does what this story asks of the others: what the project file admits is compared with the base revision, and nothing is run while one differs (`crates/takt/src/validator.rs:350-373`).
- Related: `story:judge-guard` guards the judge surface of takt's own repository, at merge time only. This story covers every repository a run works on, at the step.

- Covered elsewhere: a judgement step that runs `aep plan artifact evidence … --kind test_result` in the tree writes a forged store record that takt would commit as the step's change. Found by the first adversary pass on story:run-record-and-story-evidence at 8ce2019 and reproduced in a test; fixed in that story's correction 1, where a judgement step writing under `.engineering/planning/` or `.engineering/evidence/` is refused and reverted by the step guard. This story keeps the validators' own inputs (`.cargo/**`, the test command's script, the CI workflow).

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- A test with a fake harness whose `implementation.edit` writes `.cargo/config.toml` containing `[target.'cfg(all())']` `runner = "true"` into a repository whose only test fails, the run's test command `cargo test --quiet`: the case records `tests.run` `pass` at the step's revision.
- The same with a fake harness that rewrites `./check.sh` to `exit 0` and the test command `./check.sh`: the case records `tests.run` `pass`.

## Scope

- `crates/takt/src/run.rs`: the step refusal list (cited)
- `crates/takt/src/validator.rs`: `TestsRun::observe` and the CI validator, inputs read at the base (cited)
- `crates/takt-cli/src/main.rs`: the judge-surface list per repository, `takt approve` naming touched judge paths (inferred)
- `crates/takt/tests/`: a fake-harness forgery test (inferred)

## Acceptance

A step that changes `.cargo/config.toml`, the test command's script or a file under `.github/workflows/` earns no `pass` or `ci.green` record at its revision: either the validator ran those inputs from the base revision and recorded the real result, or it refused to record and named the changed paths; and a step touching a path in the repository's judge-surface list (default `.cargo/**`, `.github/**`, `.engineering/**`, `rust-toolchain*`, `build.rs`) is refused or listed in the pull-request body and in `takt approve`.
