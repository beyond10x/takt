# Wave 57: a run on a schedule, and one served Connectors client

Stage: **in review**. `wave/57` branches from 71e8daa9, the plan of epic:scheduled-runs-and-connector-actions, and
merges `main` at 5f782fff (pull request 64: the plan, the owner's decision on connector writes and the spec-first
rule) as d6d7043f. Approved: approval-record:wave-57-scheduled-runs.

## Selection

The first wave of epic:scheduled-runs-and-connector-actions (design `docs/design/scheduled-runs-and-connector-actions.md`
§ 7). The plan-critic panel approved the set (review-result:scheduled-runs-acceptance-round-3,
review-result:scheduled-runs-design-round-2, review-result:scheduled-runs-parallel-round-2,
review-result:scheduled-runs-scope-round-2).

## Units

| unit | story | objective | scope | depends on |
|---|---|---|---|---|
| a | story:a-run-on-a-schedule | `takt run --every` with `--window`, `--time-zone`, `--until`; the run file's `schedule`; tick files; `--outcome-file` | new `schedule.rs`, `run_config.rs`, `main.rs`, `Cargo.toml`, `ess/` | |
| b | story:one-served-connectors-client | the served Connectors route out of the forge into `connectors_client.rs` | new `connectors_client.rs`, `forge/connectors.rs` | |

Both add one `pub mod` line to `crates/takt/src/lib.rs`; unit a was committed first.

## Delivery

- Unit a: 11ea5091. Its unit tests ran red first against a `todo!()` slot function (5 failed), then green.
- Unit b: f1a11c04. Its three client tests ran red first against stubs (3 failed), then green; the 12
  `connectors_forge` tests pass with the same names before and after.
- c7fa8ab8: the schedule's nouns in `ess/domains/takt.yaml` and the citations both units moved.
- ebcbe42c: the run file's refusal named `schedule`'s members in backticks, which `config_docs_adv_carry`
  reads as top-level keys; reworded.
- 2c1cb190: the configuration reference and the run-configuration guide, by the doc-writer.

## Review

One adversarial pass at 2c1cb190 (review-result:w57-adv-1). It found seven defects, all introduced in this wave. It
wrote four failing tests (`crates/takt/tests/w57_adv_schedule.rs`, `crates/takt-cli/tests/w57_adv_schedule_cli.rs`)
and confirmed that the client move kept every safety property of the forge code. All seven are fixed in a68eb760:

- Two starts at once both ran the schedule. One start now holds an exclusive lock for its life.
- The outcome file was not written when the argument parser refused the command line. It is now.
- `every` had no upper bound. It is now one second to seven days, and the slot arithmetic is checked.
- A signal could still start one tick. The signal count is read again before a tick is spawned.
- The outcome and state files were written in place. They are now written to a temporary file and renamed.
- A test's name claimed it checked the deadline. It is renamed, and a deadline test is added.

The spec-first rule (owner, 2026-11-12) reached this fix round after its code was written, so its `ess/` edits followed
the code. From wave 58 on, `ess/` comes first.

Tests after the fixes, each command run package-scoped:

| command | result |
|---|---|
| `cargo test -p b10x-takt --test w57_adv_schedule` | 4 passed |
| `cargo test -p b10x-takt-cli --test w57_adv_schedule_cli` | 2 passed |
| `cargo test -p b10x-takt --lib schedule::` | 7 passed |
| `cargo test -p b10x-takt-cli --test run_schedule_cli` | 16 passed |
| `cargo test -p b10x-takt-cli --test run_config_cli` | 16 passed |
| `cargo test -p b10x-takt --test config_docs_adv_carry` | 4 passed |
| `cargo test -p b10x-takt-cli --test config_docs_adv_carry` | 1 passed |
| `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` | exit 0 |
| `scripts/check-citations --require-digests` | 1595 resolved, 0 unresolved |

Docs after the fixes: 81786986 (the lock, the range of `every`, the outcome file).

## Gate

No local `cargo test --workspace` or `task check` ran: the host's disk was short, and the workspace test run was
stopped before its first test. The full gate is the pull request's CI (`check.yml`: check-static, test-library,
test-cli).

## Close

After the merge, a store-only close moves both stories to `implemented` with the CI run as their `test_result`. It
rides with wave 58.
