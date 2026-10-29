# Wave 39: the rsi findings of the 0.17.0 trial

Stage: **closed**, waiting to merge. Shape: one dispatcher in one managed worktree (`takt-wave-39`), one wave branch
`wave/39` cut from `main` at 13f7f83, one shared target directory, builds one at a time. Approved
by the orchestrator under the owner's grant. The wave merges into `integration/takt` once the
real-story deliveries running there have finished, not into `main` directly.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:failed-gate-output-floods-prompt | `crates/takt/src/validator.rs`, `harness.rs`, `crates/takt/tests/failed_gate_lines.rs` | baa9c6ee, c343d0ba, 7c9e7129 |
| U2 | story:unqualified-stop-records-no-layer | `crates/takt/src/scorecard.rs`, `run.rs`, `crates/takt/tests/layers.rs` | 175d2320 |

## Log

- U1 and U2 committed; the adversarial review approved U2 and returned four findings on U1.
- U1 fix round 1 (c343d0ba): all four fixed. `task check` at c343d0ba: specification, citations,
  release notes, format and lint exit 0; the workspace test, `--no-fail-fast`, 2337 passed, 0
  failed, 5 ignored. A first run had stopped at one failure of
  `harness::tests::failed_checks_pass_the_scan_scope_files_pass`, a test older than this wave,
  which passed in three runs of the library's tests after it (170 passed each).
- The reviewer's confirmation at c343d0ba: approve with fixes. An unterminated private key block
  hides the failed tests after it, uncounted; `failure_line` keeps `warning:`, `note:`, `info:`
  lines and timestamps. Fix round 2 takes both.
- Fix round 2 (7c9e7129): the reviewer approved. `task check`'s commands at 7c9e7129:
  specification, citations, release notes, format and lint exit 0; the workspace test,
  `--no-fail-fast`, 2338 passed, 2 failed, 5 ignored. Neither failure is in the wave's code:
  `a_forge_wait_longer_than_the_active_budget_does_not_stop_the_run` spent 2.1 s of a 2 s active
  budget at a load average of 24 to 40, and an `approval_riders_adv2` case was refused because
  another process installed a program into a directory takt guards during the run. Both test
  files passed alone at 7c9e7129 (3 and 10 passed). An earlier run at the same sha had 3 more
  failures from a sibling tree's build replacing the `takt` binary in the shared target
  directory mid-run; builds of the three trees now take one lock.
- Both stories moved to implemented on that run.

Filed at close: story:credential-shape-sees-a-token-after-a-word-character (rsi, high),
story:failed-gate-keeps-other-linter-shapes (rsi, low), story:scan-scope-test-is-not-flaky
(medium), story:validator-only-run-records-no-layer.
