---
format: aep.planning-md/3
id: review-result:variety-runs-a0b8576
kind: review-result
status: active
title: 'Reviewer: four variety runs on public beyond10x tasks, takt a0b8576'
relations:
- reviews: story:no-model-step-for-approval
prose_only: a trial report with run figures and diagnosed defects, not a plan critic verdict
revision: 1
---
Four of the five runs are done; the fifth was never started, and the fourth run's hidden acceptance was not run, because you said to start no new builds. All four runs stopped at the merge approval (exit 4) with no resumes. I found two new takt defects, and the more serious one shows up in every run.

takt: `the installed takt`, sha256 `b7af9ec9f681d8c87233dc0337a77c4ae20da2a2f51fa6e4586a2cec45d35c56`. `takt --version` prints `takt 0.1.0` with no sha, so it is the turn-1 build from a0b8576. Every run used `--forge none --harness claude --iterations 30 --hours 0.5`.

| # | repository, unit | seed | intent from | test command | stop line | exit | steps | tokens | wall s | resumes | hidden acceptance | diff --stat (case files excluded) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | metaharness `9ca7d3d` (not a merge; module plus refactor) | `50c9789` | commit title plus its `website/README.md` paragraph | `cargo test --quiet -p metaharness-docs --locked` | ApprovalRequired (repository.merge) | 4 | 6 | 473,816 | 224 | 0 | **pass**: publication 6/6 (5/6 on the seed) | 3 files, +357 −19 |
| 2 | worktree `26265f5` (CLI flag `--scope`) | `12e430c` | story body, scope-flag-for-reporting-subcommands | `cargo test --quiet --workspace --locked` | ApprovalRequired (repository.merge) | 4 | 6 | 935,443 | 326 | 0 | **pass**: whole workspace exits 0; `gc_scope` was 1/4 on the seed | 4 files, +359 −6 |
| 3 | canon `98f7118` (new module, schema classifier) | `92c95fd` | story:action-arguments § Versioning, the dialect, and the `classify` signature | `cargo test --quiet -p b10x-canon --lib --locked`, with `RUST_MIN_STACK=16777216` | ApprovalRequired (repository.merge) | 4 | 4 | 727,899 | 371 | 0 | **fail**: 208 pass, 1 fails | 3 files, +524 |
| 4 | worktree `82e998c` (bug fix: reconcile after a deleted repository) | `614f1b0` | the fix commit's body, without its process lines | `cargo test --quiet --workspace --locked` | ApprovalRequired (repository.merge) | 4 | 6 | 1,631,823 | 371 | 0 | **not run** (disk stop) | 3 files, +409 |
| 5 | worktree `dfb808c` (cache rule for `target/tmp`) | `db00e2c` | story body, cargo-target-tmp-is-cache | — | not started (disk stop) | — | — | — | — | — | the hidden test fails on the seed (8/10) | — |

**Why run 3 failed.** The one failing test is `a_keyword_outside_the_dialect_in_a_changed_schema_is_changed`. That rule is stated in the unit's module doc, and I left it out of the intent I wrote. It is a trial-setup error on my side, not a takt defect and not the model doing the task badly.

**Setup deviations:**
- On this machine canon's full suite fails at the seed: one test overflows the stack, and another refuses because the installed `ess` is 0.60.0 while the tree wants 0.59.0. So run 3 used the narrower command and the larger stack shown above.
- Run 3's first attempt was correctly refused with exit 1. A core dump from my warm-up build was left in the clone.

## Defects

| # | severity | file:line (main; a0b8576) | defect |
|---|---|---|---|
| D1 | medium | `crates/takt/src/case.rs:1364` (a0b8576: `:1098`) | When a case opens, `implementation` is bound to the base HEAD. A passing test run on the unchanged base therefore makes `implementation.verified` true, and `repository.merge` becomes approval-required before any change exists. |
| D2 | low | `crates/takt/src/validator.rs:281-305` (a0b8576: `:148`) | The `tests.run` summary keeps only the last 5 lines of stdout plus stderr. In a Cargo workspace those are the doc-tests of the last crate, so every passing run reads "running 0 tests … 0 passed". |

**D1** happened in all four runs: `decisions/0002.json`, written right after step 1, already has `repository.merge` approval-required with zero commits.
- Reproduction: `git init` a repository with one commit, then `takt run --repo <it> --intent "add a CHANGELOG entry" --forge none --harness none --iterations 3 --test-cmd true`.
- Observed: exit 4 and `stopped: ApprovalRequired (repository.merge)` after 2 steps, with `git log main..HEAD` empty.
- Possible fixes (my inference, not tested): bind `implementation` only once the case branch has its own commit, or change the protocol's claim.

**D2** shows up in runs 1, 2 and 4: an operator or the model cannot tell from it how many tests ran.
- Reproduction: `takt run --repo <the reviewer's scratch directory> …` with `cargo test --quiet --workspace`, then read the step 1 effect line.
- Observed: `the test command exited 0: pass;  | running 0 tests |  | test result: ok. 0 passed; …`. The real run passed 26 test suites.

**Already filed, not new:** a model step spent only to request the merge approval cost 65,591 + 64,114 + 75,249 + 127,803 = 332,757 tokens over the four runs (story:no-model-step-for-approval, active).

I approved the run 2 case in its scratch clone to check that the untracked case directory does not block `takt approve`: it does not (exit 0). That case was not resumed. Nothing is running now, and nothing was deleted. When disk allows, run 4's acceptance is `<the reviewer's scratch directory> worktree-2 worktree 82e998c crates/worktree-cli/tests/deleted_repository.rs cargo test --quiet --workspace --locked`, and run 5 is `bin/run.sh worktree-3 …` with `intents/worktree-3.txt`.

Files are in `<the reviewer's scratch directory>`:
- run logs: `logs/<name>.r0.log`
- acceptance logs: `logs/<name>.accept.log`
- cases: `<name>/.engineering/cases/`
- intents: `intents/`
- D1 reproduction: `repro-seedmerge/`