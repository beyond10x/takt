# Wave 40: rsi first, a benchmark protocol, and a story takt delivered itself

Stage: **closed**, waiting to merge. Shape: one dispatcher in one managed worktree (`takt-wave-40`),
one wave branch `wave/40` cut at 338082d (the merge of `integration/takt` with release 0.18.0), one
shared target directory, builds one at a time. Approved by the orchestrator under the owner's grant:
approval-record:store-replan-2026-10-28 (wave 40 goes rsi first; every second or third story is
delivered by takt itself) and approval-record:wave-39-merges-now (waves 40 and 41 follow wave 39,
each when it closes green). The wave merges into `integration/takt`, whose batch has one full gate;
this wave ran its targeted tests only.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:bench-task-protocol | `protocols/bench-task/1.yaml`, `crates/takt/tests/fixtures/bench-task/`, `crates/takt-cli/tests/bench_task_cli.rs`, `AGENTS.md` | 7be207a5 |
| U2 | story:the-cli-suite-fits-its-ci-budget (rsi) | `.github/workflows/check.yml`, `crates/takt-cli/tests/takt_gates_document.rs` | c6408f6f |
| U3 | story:a-turn-pins-the-repository-of-its-forge (rsi) | `crates/takt/src/case.rs`, `improvement.rs`, `crates/takt-cli/src/main.rs`, `ess/`, tests | bc4be287, d490b165, b0dd8047 |
| U4 | story:a-duplicate-yaml-key-is-refused-at-its-line | `crates/takt/src/run_config.rs`, tests | 7ca2db5d (written by takt), dcae6b06 |
| citations | | `ess/domains/takt.yaml`, `takt.citations.tsv` | 86b571ea |

`git diff --shortstat 338082d8 b0dd8047`: 68 files changed, 5338 insertions, 459 deletions; 21 of
them outside `.engineering/` (4080 insertions, 289 deletions).

## Log

- 2026-10-28: U1 (7be207a5). `bench.task/1` declares no forge action and no action that requires a
  capability; its acceptance run on the fixture task exits 0 with `stopped: Completed (completed)`.
  `bench_task_cli`: 2 passed, 0 before (the commit's body).
- 2026-10-28: U2 (c6408f6f). The check jobs' `timeout-minutes` is back at 30, and a test asserts
  it. Over 11 green runs of check.yml on 2026-10-27/28, test-cli took median 488 s, max 611 s, and
  test-library, the slowest job, median 693 s, max 1062 s, under two thirds of 30 minutes (1200 s)
  (the commit's body). `takt_gates_document`: 8 passed.
- 2026-10-28: U4 delivered by takt itself, on a scratch clone, `--forge none`, the claude harness.
  Run 1 on 0.17.0 stopped `RepeatedTestFailure` (exit 3) after 5 steps and 207955 tokens: 0.17.0
  writes no `tests_change` record, so `implementation.edit` stayed blocked after the red test (fixed
  in 0.18.0). Run 2 on 0.18.0 wrote the test, ran it red, edited and ran it green, then its gate
  failed on format and citations, and the run ended `Declined` (exit 9) after 9 steps and 421010
  tokens: after a red gate with green tests, story.delivery/2 offers only `tests.write`. Figures from
  the dispatcher's run log. Its diff entered the wave as 7ca2db5d with the format fixed by rustfmt.
- 2026-10-28: the adversarial pass on U4 returned needs-change: the hand-written scanner ran on every
  YAML error, read text inside scalars as keys, and its test passed on the generic refusal. Fix
  round 1 (dcae6b06) names a duplicate only from serde_yaml_ng's `duplicate entry` error, and TOML's
  key too: `run_config_adv1` 6 passed, the new `run_config_dup_adv2` 10 passed (8 red before),
  `--lib` 170 passed (the commit's body). Three limits are left: an alias key, a tagged key and a
  plain key ending in a colon are refused at the mapping's first line without the key's name; filed
  as story:duplicate-key-reader-limits (rsi, low), its predicate the 3 ignored cases of
  `w40_adv_dup_and_pin.rs`.
- 2026-10-28: U3 (bc4be287). A turn's forge file records `bot <owner>/<repo>`, a grant records the
  repository its mainline tip was read from, and a resume, a grant or a merge read from another
  repository is refused, naming both. 86b571ea re-points the citations the wave moved and cites U3's
  sentences: 1311 of 1311 resolved.
- 2026-10-28: the wave's adversarial pass returned needs-change on U3: a grant given before grants
  recorded a repository was refused with "grant it again", which `takt approve` then refused. Fix
  round 1 (d490b165) lets such a grant be given again where the turn reads a repository, and adds
  the adversary's files `w40_adv_turn_pin.rs` (3) and `w40_adv_dup_and_pin.rs` (3, and 3 ignored).
- 2026-10-28: the re-verification at 407b4e0b returned needs-change with one warning: when main of
  the turn's repository already held the candidate, "grant it again" led to a grant the next resume
  refused for good. Fix round 2 (b0dd8047) says no grant given now authorises that merge, and adds
  `w40_adv2_regrant.rs`.
- 2026-10-29: targeted tests at b0dd8047: `self_improvement_cli` 57 passed, `w40_adv_turn_pin` 3
  passed, `w40_adv2_regrant` 6 passed; clippy exit 0; `check-citations --require-digests` 1311
  in-repository, 1311 resolved, 0 unresolved.
- 2026-10-29: `scripts/check-finding story:a-turn-pins-the-repository-of-its-forge` gave
  `reproduced` at the promoted revision f4f8d8d2 and `error` at b0dd8047: the predicate's seed
  carries a `check-finding` script the candidate no longer accepts, a protocol file since removed
  and the old cases path, so the turn does not start. Filed as
  story:turn-pin-predicate-runs-on-current-takt (rsi, medium); U3 stays active until it runs.
- 2026-10-29: two stories moved to implemented, each on a test_result record on its commit's
  tests, whose files are unchanged since: U1 and U4. U2 stays active: its acceptance 3 asks for
  three consecutive runs of `check.yml` on main under 20 minutes with `timeout-minutes` at 30,
  and its change reaches main with this batch's pull request. The move shifted one line of `bench-task-protocol.md` that `ess/` cites;
  `check-citations fix` re-pointed it, and the check then read 1311 in-repository, 1311 resolved,
  0 unresolved.

Filed in the wave: story:duplicate-key-reader-limits (rsi, low, draft),
story:turn-pin-predicate-runs-on-current-takt (rsi, medium, draft).
