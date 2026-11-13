# Wave 59: a connector read action

Stage: **merged**. Pull request 67 merged at b47913d3 as 32d02632 (approval-record:pr-67-merge); released in 0.36.0. `wave/59` branches from `wave/58` at 67eea205 and merges `wave/58` at 933de0d4 (its CI fix) as
d0e4f4ec. Approved: approval-record:wave-59-connector-read-action. The last wave of the epic for now: waves 60 to 62
are not started (`docs/handoff/2026-11-13.md`).

## Selection

The third wave of epic:scheduled-runs-and-connector-actions (design `docs/design/scheduled-runs-and-connector-actions.md`
§ 4 "Read action" and § 7): story:a-connector-read-action, on wave 57's served client and wave 58's run-file binding.

## Units

| unit | story | objective | scope | depends on |
|---|---|---|---|---|
| a | story:a-connector-read-action | `connectors` and `connector_actions`: a protocol read action bound to a Connectors operation | new `connector_action.rs`, `run_config.rs`, `main.rs`, `ess/` | story:a-command-action |

## Delivery

Spec first: each change to behaviour has its `ess/` commit before its code commit.

- f4635da6: `takt.work.ConnectorsEndpoint`, `takt.work.ConnectorAction` and the fields `connectors` and
  `connector_actions` on `takt.work.RunConfiguration`.
- 69e557d5: the connector read action. `connector_action_cli` ran red first (6 failed), then green.
- 51be1523: the shared source gates at gates 0.1.27, which scans in batches with no total cap.
- ed55ebdf: `ConnectorAction` joins the run-file-bound validators in `pr_opens_when_done`.

## Review

One adversarial pass at ed55ebdf (review-result:w59-adv-1). Four defects, all introduced in this wave, each with a
failing test in `crates/takt-cli/tests/w59_adv_connector_action_cli.rs`. The specification of the fixes and the fixes
follow it:

- Every answer that was not a success was recorded `fail`: a stale descriptor, a gateway's page, a redirect. Only the
  instance's own refusal, an error envelope whose code is not `stale_description`, is `fail` now; anything else is an
  error with no record, and the next run or tick describes again.
- The call was held to the client's 120 seconds, not to the run's hours. It is held to the smaller of the two.
- An action bound in both `command_actions` and `connector_actions` ran the command binding. It is refused at the
  later binding's line, exit 2.
- The kept answer was pretty-printed and could pass the 16 MiB the client reads. It is written compact.

Tests after the fixes, each command run package-scoped with `CARGO_INCREMENTAL=0`:

| command | result |
|---|---|
| `cargo test -p b10x-takt-cli --test connector_action_cli` | 7 passed |
| `cargo test -p b10x-takt-cli --test w59_adv_connector_action_cli` | 6 passed |
| `cargo test -p b10x-takt-cli --test run_config_cli --test connectors_forge_cli` | 16 and 4 passed |
| `cargo test -p b10x-takt --lib` | 269 passed |
| `cargo test -p b10x-takt --test connectors_forge --test effects --test confinement --test pr_opens_when_done` | 12, 7, 22 and 15 passed |
| `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` | exit 0 |
| `scripts/check-citations --require-digests` | 1627 resolved, 0 unresolved |

## Gate

No whole-crate or workspace test ran locally: the host's disk. The full gate is the pull request's CI.

## Close

Pull request 67 merged at its granted head b47913d3 as 32d02632, its seven checks green (`check` run
https://github.com/beyond10x/takt/actions/runs/43346440639). story:a-connector-read-action moved to `implemented` with
that run as its `test_result`. An independent review of b47913d3 found no blocker; its findings F1 to F8 are filed
as approval-record:pr-67-merge lists, and the documentation of F1, F3, F4 and F7 went into the 0.36.0 release prepare.
