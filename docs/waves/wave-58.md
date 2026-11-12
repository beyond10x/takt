# Wave 58: a command action

Stage: **in review**. `wave/58` branches from `wave/57` at f4616dbd while pull request 65 waits for its grant, and
merges `wave/57` at dabcd934 (its CI fix) as 1160b381, and `main` at 2c1a3a31 (pull request 65) as 33066b1a. Approved: approval-record:wave-58-command-action.

## Selection

The second wave of epic:scheduled-runs-and-connector-actions (design `docs/design/scheduled-runs-and-connector-actions.md`
§ 7): story:a-command-action, with the registration of run-file-bound validators that story:a-connector-read-action
reuses.

## Units

| unit | story | objective | scope | depends on |
|---|---|---|---|---|
| a | story:a-command-action | `command_actions`: an argv bound to a protocol action, its exit status as evidence | new `command_action.rs`, `validator.rs`, `run_config.rs`, `main.rs`, `ess/` | |

## Delivery

Spec first: each change to behaviour has its `ess/` commit before its code commit.

- 5ffd25eb: `takt.work.CommandAction`, `takt.work.CommandActionResult` (`pass`, `unchanged`, `done`, `fail`) and the
  field `command_actions` on `takt.work.RunConfiguration`.
- d123c40c: the command action and the generic registration (`RunFileBinding`, `RunFileBound`, `register_bound`).
  `command_action_cli` ran red first (4 failed), then green. The run-configuration guide and `config_docs_adv_carry`
  follow the new refusal text.
- e05f05e4: the wave's open in the store, and the acceptance `connector_write_outcome_unknown_not_resent` on
  story:a-connector-write-under-a-grant: the served Connectors host answers 502 `outcome_unknown` when its own
  pre-send guard refuses and nothing was sent, so a write case records `fail` and never resends.

## Review

One adversarial pass at e05f05e4 (review-result:w58-adv-1). Four defects, all introduced in this wave, each with a
failing test (`crates/takt-cli/tests/w58_adv_command_action_cli.rs`, `crates/takt/tests/w58_adv_command_action.rs`),
and one note. The specification of the fixes is 794de514, the fixes a81a3d3e:

- A binding could record evidence of a kind its action may not produce: a `test_result pass` with the test command
  `false` completed a case. Every run-file binding is now refused at its line, exit 2, when its action's
  `may_produce` does not name its kind.
- The declared output left by the child as a link out of `artifacts/` was kept. The output and each directory from
  `artifacts/` to it must now be its own: no symbolic link, no other kind of file, no hard link.
- The child was not held to the run's hour budget. It now is, as the test command's child is.
- The reference said "those eleven" of twelve fields.
- Note: `takt scenario run` checks `command_actions` and applies none; the reference now says so.

The reviewer's confirm at d326c51d (review-result:w58-adv-2) found one more defect, introduced in this wave: the
kind check ran only when a case opened, so `takt run --case` with a binding of another action's kind still recorded
`test_result pass`. The specification is a2865232, the fix 7eaa28ca: a resumed case runs the bound validators' open
checks and refuses exit 2, and the command action checks the kind against the case's pinned protocol at each
observation, before it runs anything.

Tests after the second fix round, each command run package-scoped:

| command | result |
|---|---|
| `cargo test -p b10x-takt-cli --test command_action_cli` | 6 passed |
| `cargo test -p b10x-takt-cli --test w58_adv_command_action_cli` | 4 passed |
| `cargo test -p b10x-takt --test w58_adv_command_action` | 1 passed |
| `cargo test -p b10x-takt --lib` | 267 passed |
| `cargo test -p b10x-takt --test confinement --test effects --test tests_have_a_home_of_their_own` | 22, 7 and 4 passed |
| `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` | exit 0 |
| `scripts/check-citations --require-digests` | 1607 resolved, 0 unresolved |

## Gate

No local `cargo test --workspace`: the host's disk keeps builds package-scoped. The package-scoped runs include the
tests that scan every source file. Wave 57's first CI run failed two of them (`confinement`,
`tests_have_a_home_of_their_own`). On this wave `effects::no_record_reaches_a_case_but_through_the_case_evidence_port`
failed: two lines of `command_action.rs` read the binding's `evidence` key in the run file, which the scan reads as a
path into a case. They write no record; both are now listed in its `EVIDENCE_LINES` with what they do.
`.agents/dispatcher-agent.md` step 5 now names the three tests. The full gate is the pull request's CI.

## Close

After the merge, a store-only close moves story:a-command-action to `implemented` with the CI run as its
`test_result`. It rides with wave 59. Wave 57's close rides with this wave (3ccfd674).
