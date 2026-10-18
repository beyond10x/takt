# Trial 1: the promoted takt on a story-sized unit of the worktree repository

One takt run, 2026-10-18, on a unit already merged in github.com/beyond10x/worktree, with the
unit's own test file held out. Every figure below is read from a file in the trial directory, named
in the last column; times are UTC. The row in `docs/metrics/scenario-runs.jsonl` is
`headstart-trial-1`.

| item | value | source |
|---|---|---|
| takt | the installed binary, sha256 `b7af9ec9f681d8c87233dc0337a77c4ae20da2a2f51fa6e4586a2cec45d35c56`, the build of a0b8576 that improvement turn 1 promoted | `binary.txt`; `promoted_at` in `docs/metrics/improvement-turns.jsonl`; the same sha256 in the notes of T-14..18 |
| task | `story:cargo-target-tmp-is-cache` of the worktree repository, seed `db00e2c`, the first parent of the unit's merge `dfb808c`; the intent is the story's Outcome, Why, Specification, Contract and Acceptance sections at the seed | `intent.md`; `git log` in `tree/` and of the worktree repository |
| held out | the unit's test file `crates/worktree-cli/tests/cache_discard.rs` at `dfb808c`: 10 tests, 4 of them added by the unit; never shown to the run | `acceptance.sh`, `hidden-tests.patch`, `acceptance.test.log` |
| command | `takt run --repo . --intent <intent.md> --forge none --harness claude --model claude-opus-5-5 --hours 1 --test-cmd "cargo test --locked --workspace --quiet"`, protocol `software.change/1`; the tests were compiled with `cargo test --no-run` before the run | `run.sh`, `run.log`, `prebuild.log` |
| stop | `ApprovalRequired (repository.merge)`, exit 4, 6 steps, 1 invocation, no resume | `result.txt`, `run.log` |
| wall time | 242 s, 20:55:20 to 20:59:22 | `result.txt` |
| tokens | 592,698, takt's stop line; per call (input + cache write + cache read + output): 109,895, 369,169 and 113,634 | `run.log`; `steps.jsonl` |
| model calls | 3, all claude-opus-5-5: `tests.write` (7 turns, 41.9 s), `implementation.edit` (19 turns, 64.7 s), and the call that only asks for the merge approval (7 turns, 23.6 s) | `steps.jsonl` `num_turns`, `duration_ms`, `modelUsage` |
| cost | $0.849: $0.279 + $0.407 + $0.164 | `steps.jsonl` `total_cost_usd` |
| validators | `tests.run` three times: 19 s (20:55:20 to 20:55:39, pass at the seed), 26 s (20:56:23 to 20:56:49, fail: 6 passed, 2 failed in `cache_discard`), 62 s (20:57:55 to 20:58:57, pass) | `events.jsonl` |
| held-out check | pass: 10 passed, 0 failed | `acceptance.out`, `acceptance.test.log` |
| commits of the run | `9df7bac` `tests.write` (1 path), `0dcf8f5` `implementation.edit` (2 paths), on the case branch `takt/takt-20261018T205520Z` cut from `trial/0` at the seed | `git log` in `tree/` |

Where the time went, from `events.jsonl` and `steps.jsonl`: the validators took 107 s and the
model calls 130 s, 23.6 s of it the approval-only call. The remaining 5 s is takt and the gaps
between steps.

The variety run `worktree-3` ran the same unit from the same seed. It differs in three ways: no
`--model`, no prebuilt tests, and the intent was written separately. That run stopped `NoProgress`
once, and one resume took it to the approval: 689,874 tokens, 313 s.

## Defects seen

1. **The `tests.run` summary keeps only the last result block.** Steps 1 and 5 record `running 0
   tests |  | test result: ok. 0 passed; 0 failed`, yet the workspace's test run in
   `workspace-test.log` printed 20 result blocks and 162 passes (`workspace-test.summary`). The
   finding and its predicate are in `finding-tests-run-summary.md`. The predicate was run by hand
   with the promoted binary (`probe.sh`; `probe.result`: the output matched). It is the defect
   `story:test-summary-keeps-the-totals` names.
2. **A model call that only asks for the merge approval:** 23.6 s, 113,634 tokens and $0.164 of
   this run. Already filed as `story:no-model-step-for-approval`; this run is one more data point.

## Method note

The first acceptance script applied the unit's test diff as a patch, and the patch conflicted
because the run had edited the same test file (`acceptance.apply.err`). Replacing the whole file
with the unit's version worked. From now on a hidden test is a file that overwrites the
candidate's, never a patch on it.
