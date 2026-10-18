---
format: aep.planning-md/3
id: review-result:adversary-integration-pass-1
kind: review-result
status: active
title: Adversary, wave 06 integration fixes, pass 1
relations:
- reviews: story:validator-confinement
- reviews: story:tree-hygiene
- reviews: story:authority-hardening
- reviews: story:cli-protocol-defaults
revision: 1
---
unit: wave 06 integration fixes, impl/integration-fixes at e48695d (worktree = e48695d plus 2 untracked test files)
verdict: CONFIRMED (pre-existing, 2); the 2 failing cases this diff introduced are INFEASIBLE, both in states I built by hand
cases: executed 269→273, red 4
origin: introduced 3 / pre-existing 2 / undecided 0
wrote-outside-worktree: 3 paths under <wave-scratch>/scratch/int/adv1/ (part 6)
needs-coordinator: is a case whose take is on the base but not on its own branch a state takt supports? `protocol_defaults_adv2.rs` treats it as one ("an operator who merged it by hand"); finding F2 depends on the answer.

**Short answers to the brief**
- **(A) On its own case's branch:** nothing a step, test command or gate command does can produce a commit that `approvals_only` accepts.
  - Both the judgement-step path and the validator path refuse any move of `HEAD`.
  - A step's commit subject is always `<action>: <summary>`, and the action must be one the run offered.
  - The shipped harness has no Bash, and `--restricted` blocks writes to git files.
- **(A) On other branches:** a test command can move any ref other than `HEAD`, including a sibling case's branch and the base (F4). That is how such a commit gets onto a sibling's branch.
- **(A) Merged result:** the approve commit cannot change what is merged. `PrMerged` merges with the head pinned to the granted revision (validator.rs:1011).
- **(A) The premise is wrong, though:** `approvals_only` assumes the approve commit "changes nothing a validator observes" (case.rs:743). `pr.open` does observe it (F1).
- **(B):** `main.rs` no longer calls `open_next`. `next_to_resume` is read once, then the run either resumes that case or calls `take_next`, so the check and the run cannot disagree. What changed is behaviour on a case branch (F2).

**1. `git --no-pager diff --stat`**
Empty: the tree matches e48695d. `git status --short` shows only my two test files:
```
?? crates/takt-cli/tests/integration_adv1.rs
?? crates/takt/tests/integration_adv1.rs
```
Neither file is an implementation file. No charter violation.

**2. Cases added. Each was run alone first and is red now; the panic lines below are verbatim from those runs.**

| case | asserts | at e1d75ec (base, scratch clone) |
|---|---|---|
| takt-cli `adv1_an_approval_before_the_push_lets_the_resumed_run_open_the_pull_request` | Protocol file where `pr.open` requires `repository.push`. Gate → `takt approve` → the next run opens the pull request | red earlier, at line 279: base does not commit the record |
| takt-cli `adv1_a_plain_run_on_the_cases_own_branch_resumes_the_case_the_base_lists_active` | Tree on the open case's own branch, story active on `main`: a plain run resumes the case | **green**, so introduced |
| takt `adv1_a_step_that_forges_an_approval_commit_on_a_sibling_cases_branch_is_refused` | A step writes an approve-shaped commit onto a sibling's branch and should be refused | red |
| takt `adv1_a_test_command_that_moves_the_base_branch_is_refused_and_the_base_put_back` | A test command moves `main`; it should be refused and `main` put back | red |

```
the push the approval was given for never happens: exit 5
stopped: error: pr.open (takt.pr-open): the tree's HEAD 45db5f02… is not the implementation revision d4959bf8…; the pull request's head would not be the revision the claim is about
---
the open case of the active story is not resumed: exit 3
stdout frontier: took story:ready-two: Make the second check pass
---
the step was not refused for moving takt/story-ready-one-20261018T112834Z from 1efab2c7… to 32ec0739…: Ok(NoProgress)
the sibling resumed at HEAD 32ec0739… keeps the implementation revision Some("1efab2c7…")
---
assertion `left == right` failed: the test command moved the base branch main and the run kept it: Ok(Iterations(1))
```

**3. Suite run, after the cases existed**
- Command: `cargo test --workspace --locked --no-fail-fast`. Log: `.../adv1/suite.log`. `EXIT=101`.
- Every pre-existing binary passed, 269 tests. Only my two binaries failed:
```
error: 2 targets failed:
    `-p b10x-takt --test integration_adv1`
    `-p b10x-takt-cli --test integration_adv1`
```
- The "before" count of 269 comes from this run's per-binary lines minus my two new binaries. I did not run the suite a second time with my files left out.
- `rustfmt` and `clippy -D warnings` are clean on both files.

**4. Findings (they cover e48695d)**

| id | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|
| F1 | crates/takt/src/case.rs:743 (with validator.rs:661) | INFEASIBLE | introduced | **Measured:** after an approve commit the revision is kept while `HEAD` moves on, so `PrOpen` exits with an error on every run. **Reaches it:** a `--protocol` file whose capability gates `pr.open`. No built-in protocol does this. **Fix to name:** `PrOpen` pushes the revision rather than `HEAD`, or the check accepts `HEAD` when `approvals_only` holds. |
| F2 | crates/takt/src/run.rs:486 | INFEASIBLE | introduced | **Measured:** on a case branch, `next_to_resume` never reads the base's active list, so `take_next` takes `story:ready-two` while `story:ready-one`'s case is open. Every run ends on a case branch, so that case starves. This contradicts the `takt run --help` text "the open case of a story the planning store lists active is resumed". **Reaches it:** only the take-on-main-not-on-branch state, built by `git cherry-pick` plus `git branch -f`. |
| F3 | crates/takt/src/run.rs:882 | INFEASIBLE | pre-existing | **Measured:** a judgement step that moves a sibling case's branch is accepted, because only `HEAD`'s branch and the base are guarded. **Reaches it:** not the shipped harness (Bash disallowed, `--restricted`). The same move is reachable through F4. |
| F4 | crates/takt/src/run.rs:730 | CONFIRMED | pre-existing | **Measured:** a test command moved `main`; it was not refused and not put back. The validator path takes `Checkpoint::take`, which guards no extra ref, and never reads `change.refs`. **Reaches it:** the default `--test-cmd` (`cargo test --quiet`) runs code the model wrote. `take_next` then reads the store from that `main`, and the next case branches from it. By the same mechanism (inferred, not run) a test command can put an approve-shaped commit on a sibling's branch. |
| J1 | crates/takt/src/frontier.rs:257 | CONFIRMED | introduced | (judgement) The approval's record is committed after the granted revision, and the bot route merges that revision, so the record never reaches the base's planning store. **Reaches it:** every approve through the forge (`merge_cli.rs` `approve_then_the_resumed_run_merges` builds it). |

**5. Attacked and could not break**
- A model's commit cannot carry the `takt: approve` subject: the subject is `<action>: <summary>`.
- An approve commit that touches a path outside the store moves the revision (the implementor's own test covers this).
- `commit_record` refuses and discards non-store paths.
- `approve` refuses when the tree is off the case's branch or dirty, and leaves no lock behind.
- The paths `PrMerged.writes()` declares match what the bot route's merge writes.
- `main.rs`'s check and the run read the store once and cannot diverge; `open_next` behaves the same as `next_to_resume` followed by `take_next`.
- `is_due` now propagates errors; I found no path for a model to reach it.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/int/adv1/base/`: a clone at e1d75ec with its own `target/`, 740M. I left it in place: your rules say never to delete a `target/` by hand.
- `<wave-scratch>/scratch/int/adv1/base-run.log`
- `<wave-scratch>/scratch/int/adv1/suite.log`

No processes of mine are still running.

```findings
[
  {"file": "crates/takt/src/case.rs", "line": 743, "category": "acceptance", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "approvals_only keeps the granted revision under an approve commit, but PrOpen requires HEAD == revision, so an approval that gates pr.open (reachable only with a --protocol file) leaves every later run stopped: error"},
  {"file": "crates/takt/src/run.rs", "line": 486, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "on a case branch next_to_resume never reads the base's active list, so a plain run takes the next ready story instead of resuming the open case the base lists active, contrary to takt run --help; green at e1d75ec, reached only in a hand-built take-on-main state"},
  {"file": "crates/takt/src/run.rs", "line": 882, "category": "acceptance", "severity": "note", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "a judgement step may move a sibling case's branch, for example to an approve-shaped commit approvals_only accepts, because only HEAD's branch and the base are guarded; the shipped harness has no Bash and --restricted blocks git files"},
  {"file": "crates/takt/src/run.rs", "line": 730, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "a test command (model-written code) can move the base branch or any other ref other than HEAD unrefused and not put back, because the validator path guards no ref and ignores StepChange::refs"},
  {"file": "crates/takt/src/frontier.rs", "line": 257, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the approval record is committed after the granted revision that the forge merges, so it never reaches the base's planning store"}
]
```
