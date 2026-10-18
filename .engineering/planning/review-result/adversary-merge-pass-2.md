---
format: aep.planning-md/3
id: review-result:adversary-merge-pass-2
kind: review-result
status: active
title: Adversary, merge-under-authority, pass 2
relations:
- reviews: story:merge-under-authority
revision: 1
---
```
unit: story:merge-under-authority, commits 8953dff + 95e86ae on impl/merge-under-authority (base feed6b712fb0e56dd4959679f1d66d3107814adf), worktree takt-w5-merge plus my uncommitted test file
verdict: NEEDS-CHANGE
cases: executed 147→157, red 8
origin: introduced 8 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 directory tree, the unit's scratch adversary-2/ (part 6); the shared sccache cache
needs-coordinator: (a) the fix shape for the per-run `seq` (F-A, F-B): evaluations and record ids numbered after what is on disk, or a run id in the names; (b) whether the whole of `.engineering/cases` is the kernel's (F-C) or only the running case's directory; (c) whether a grant given during a running judgement step is to be kept (F-D), which the snapshot design cannot distinguish from F1
```

## 1. Diff stat

```
 crates/takt/tests/merge_under_authority.rs | 665 +++++++++++++++++++++++++++++
 1 file changed, 665 insertions(+)
 M crates/takt/tests/merge_under_authority.rs
```

One path, a test file. No `src/`, no store write, no git state change, no edit to `crates/takt-cli/tests/merge_cli.rs`. The 147 "before" is the implementor's `gate-c1-*.log` sum (111 + 36).

## 2. Cases added (`crates/takt/tests/merge_under_authority.rs`, appended; every existing case untouched). Each run alone before the suite; red output verbatim (trace lines cut where marked).

**`adv2_a_case_finished_by_a_resumed_run_is_not_resumed_by_the_next_run` — RED.** The acceptance flow through `open_next` on the `one-ready` store with the double: run 1 stops at the gate (3 evaluations), grant, run 2 resumes and completes (2 evaluations), run 3 must resume nothing.
```
thread '...' (2519247) panicked at crates/takt/tests/merge_under_authority.rs:1610:5:
the finished case adv2-finished is resumed again by the next run on the repository; decisions ["0001.json", "0002.json", "0003.json"]: frontier: resumed story:ready-one: Make the check pass (case adv2-finished)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 26 filtered out; finished in 0.15s
EXIT=101
```

**`adv2_a_finished_case_resumed_at_a_new_commit_is_open_again` — RED.** Gate-only case completed (`0001`, `0002`), a commit on its branch, `Run::resume` with `iterations: Some(0)`; the latest decision must be this run's.
```
thread '...' (2519462) panicked at crates/takt/tests/merge_under_authority.rs:1675:5:
the latest decision read is one the earlier run wrote, not this run's: decisions ["0001.json", "0002.json"], legitimate ["accepted"]:
case story-ready-one-20261018T083253Z on gate.only/1 in .../target/tmp/merge-2519461-adv2-reopened; inputs none
frontier: gate.run (admissible)
test result: FAILED. 0 passed; 1 failed; ... finished in 2.07s
EXIT=101
```

**`adv2_a_grant_the_harness_writes_into_another_case_does_not_merge_that_case` — RED.** Case A stopped at the gate; case B (an intent, no validators) whose harness writes `.engineering/cases/adv2-waiting/authority.yaml` and reports `repository.inspect`; A resumed with `NoHarness`.
```
thread '...' (2519812) panicked at crates/takt/tests/merge_under_authority.rs:1756:5:
the other case's harness granted adv2-waiting repository.merge and the resumed run merged pull request #1; the other case stopped NoProgress, not refused:
step 1: repository.inspect (sibling-granting)
  said: read the repository
  effect: no file changed
the waiting case stopped Completed (accepted):
step 1: repository.merge {}
  effect: merged pull request #1 at 127d291f through memory: merged
  evidence: merge_observation merged
test result: FAILED. 0 passed; 1 failed; ... finished in 0.05s
EXIT=101
```

**`adv2_a_grant_given_while_a_judgement_step_runs_is_kept` — RED.** The harness calls `grant()` on its own case during the step (standing in for `takt approve` in flight), changes nothing, reports `repository.inspect`; `authority.yaml` must still hold the grant.
```
thread '...' (2519974) panicked at crates/takt/tests/merge_under_authority.rs:1802:5:
the operator's grant given during the step is gone; the run stopped StepRefused (repository.inspect wrote into the case directory: authority.yaml):
step 3: repository.inspect (approved-meanwhile)
  said: read the repository
  refused: repository.inspect wrote into the case directory: authority.yaml; the case directory was put back and the change discarded
EXIT=101
```

**`adv2_the_restore_does_not_delete_through_a_symlink_the_harness_planted` — RED.** The harness plants `<case>/link -> <dir outside the tree>` holding `keep.txt`.
```
thread '...' (2520228) panicked at crates/takt/tests/merge_under_authority.rs:1854:5:
the kernel deleted a file outside the tree through the link the harness planted; the run stopped StepRefused (repository.inspect wrote into the case directory: link/keep.txt):
EXIT=101
```

**`adv2_a_decision_that_does_not_read_does_not_fail_the_frontier` — RED.** An empty `decisions/0001.json` in the open case of the active story; `open_next` must still answer.
```
thread '...' (2520493) panicked at crates/takt/tests/merge_under_authority.rs:1894:9:
one decision file that does not read fails every later run on the repository: read the decision document: EOF while parsing a value at line 1 column 0
EXIT=101
```

**`adv2_tests_write_does_not_commit_a_non_test_path_beside_the_cases_directory` — RED.** The harness writes `tests/new_test.txt` and `.engineering/cases-smuggled.txt`, reports `tests.write`.
```
thread '...' (2520715) panicked at crates/takt/tests/merge_under_authority.rs:1939:5:
a non-test path was committed under tests.write; the run stopped NoProgress; committed:
.engineering/cases-smuggled.txt
README.md
tests/new_test.txt
step 1: tests.write (smuggling)
  said: added a test
  effect: committed 1 path(s); implementation is now 7c142aa9
EXIT=101
```

**`adv2_a_refused_case_write_discards_the_tree_change_with_it` — GREEN** (mutant catcher for MI). `test result: ok. 1 passed`.

**`adv2_pr_merged_records_nothing_when_the_forge_still_reports_the_pull_request_open` — GREEN** (mutant catcher for MN; a `StillOpen` forge double). `test result: ok. 1 passed`.

**`adv2_a_case_resumed_at_a_new_commit_re_establishes_its_claims_before_the_merge` — RED, not predicted.** Written as the green follow-through of the F3 fix (commit between gate stop and grant, resumed run pushes again, reads, merges at the new head). It failed on its first execution for a reason I had not seen in the code:
```
thread '...' (2541467) panicked at crates/takt/tests/merge_under_authority.rs:2129:34:
the run stops without an error: evidence `pull-request-1` is given more than once
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 27 filtered out; finished in 0.04s
EXIT=101
```
The double's calls before the error were `push takt/adv2-re-established`, `view #1`: `pr.open` ran at the new revision and wrote `pull-request-1`, the id the first run's `pr.open` had already written (`validator.rs:516`, `seq` restarts at 1 in `run.rs:393`); the next `evaluate` hands Canon two records with one id. The record is on disk under `evidence/`, so every later evaluation of the case reads it: the case is unrecoverable without a hand edit.

## 3. Suite run (after all ten cases existed)

`cargo fmt -p b10x-takt -p b10x-takt-cli --check` → `fmt EXIT=0`. `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets --offline -- -D warnings` → `clippy EXIT=0`.

`cargo test -p b10x-takt --no-fail-fast --offline` (scratch `suite-takt-final.log`):
```
test result: ok. 14 passed; 0 failed; ... (unittests src/lib.rs)
test result: ok. 33 passed; 0 failed; ... (tests/forge.rs)
test result: ok. 15 passed; 0 failed; ... (tests/frontier.rs)
test result: ok. 16 passed; 0 failed; ... (tests/gate_validator.rs)
test result: ok. 2 passed; 0 failed; ... (tests/loop.rs)
test result: FAILED. 20 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.23s (tests/merge_under_authority.rs)
test result: ok. 13 passed; 0 failed; ... (tests/story_delivery.rs)
test result: ok. 0 passed; ... (doc-tests)
EXIT=101
```
`cargo test -p b10x-takt-cli --no-fail-fast --offline` (`suite-cli-final.log`):
```
test result: ok. 17 passed; ... (unittests src/main.rs)
test result: ok. 3 passed; ... (tests/forge_cli.rs)
test result: ok. 6 passed; ... (tests/frontier_cli.rs)
test result: ok. 8 passed; ... (tests/gate_cli.rs)
test result: ok. 2 passed; ... (tests/merge_cli.rs)
EXIT=0
```
Executed 121 + 36 = 157; red 8, all mine. Every pass-1 case and every implementor case is green.

## Mutants (scratch copy `adversary-2/mutant/`, sources verified equal to the worktree after each run, its `target/` (1.3G) removed by me)

| Mutant | Edit | Suite as handed (`--skip adv2_`) | Caught by |
|---|---|---|---|
| MA restore dropped (`run.rs:523`) | `restore(..)?;` → `let _ = &kept;` | red | implementor's planted-record and changed-or-removed cases |
| MF removals unseen (`run.rs:129-131`) | `before.keys().chain(after.keys())` → `after…after` | red | implementor's changed-or-removed case |
| MG additions unseen | → `before…before` | red | implementor's planted-record case, pass-1 self-grant case |
| MI `discard_all` dropped on a case-directory refusal (`run.rs:542`) | line removed | **green** | my `adv2_a_refused_case_write_discards_the_tree_change_with_it` |
| MB `is_finished` always false (`case.rs:342`) | `is_some_and(|_| false)` | red (lib + CLI) | pass-1 F2 cases |
| MJ `latest_decision` takes the first (`case.rs:330`) | `last()` → `first()` | red (lib + CLI) | pass-1 F2 cases |
| MC resume keeps the stale revision (`case.rs:585`) | `is_some_and(|_| false)` | red | pass-1 F3 case |
| MD grant accepts any capability (`case.rs:262`) | `if false && …` | red | pass-1 F4 case |
| M5 `active` not consulted (`case.rs:365`) | pass-1 edit | red | pass-1 F8 case |
| M6 resume without checkout (`case.rs:568`) | `if false {` | red | pass-1 F9 case |
| MN merge read-back ignored (`validator.rs:886`) | `(after.state, true)` → `(Merged, true)` | **green** (also `tests/forge.rs` green) | my `adv2_pr_merged_records_nothing_when_the_forge_still_reports_the_pull_request_open` |
| ME `NoHarness` never requests (`main.rs:125`) | `.first()` → `.get(1)` | red (CLI) | pass-1 F5 CLI case |

## 4. Findings (commit 95e86ae in the worktree above)

| # | file:line | what was measured | what reaches it | verdict | origin | severity |
|---|---|---|---|---|---|---|
| F-A | `crates/takt/src/case.rs:320` (`latest_decision`), with `run.rs:393` (`seq` restarts per run) and `case.rs:751` (`decisions/{seq:04}.json` overwritten) | after the resumed run completes, `decisions/` holds `0001`,`0002` (this run) and `0003` (run 1's ApprovalRequired); latest by name is `0003`, `is_finished` false, the next `open_next` prints `frontier: resumed story:ready-one … (case adv2-finished)`; the reverse in case 2: an open case reads finished | the story's own acceptance flow on a store (run 1 ≥ 3 evaluations, the resumed run 2); the implementor's F2 cases use the gate-only protocol where the completing run is the first, so they pass. Pass-1 F2 is not closed | NEEDS-CHANGE | introduced | blocker |
| F-B | `crates/takt/src/run.rs:393` with `validator.rs:116,282,516,624,916` (record ids `<kind>-{seq}`) | a resumed run at a new revision: `pr.open` writes `pull-request-1` again, the next evaluate errors `evidence pull-request-1 is given more than once`; the duplicate stays under `evidence/` | the correction's own F3 scenario (commit on the case branch between runs; "the validators run again"); with the CLI's `TestsRun` first, `test-result-1` collides the same way. Before correction 1 the stale revision kept the validators quiet; before the unit nothing resumed | NEEDS-CHANGE | introduced | blocker |
| F-C | `crates/takt/src/run.rs:519` (`snapshot(&self.case.dir)`) | case B's harness wrote A's `authority.yaml`; B's step was accepted (`effect: no file changed`), the file survived `discard_all` (`git.rs:98` exempts `.engineering/cases`) and was never committed (`git.rs:77`); A's next run merged `#1` | two case directories under one tree: `takt run --intent` or `--case-id` beside a story's case waiting at the gate; the F1 class, one directory over | NEEDS-CHANGE | introduced | warning |
| F-D | `crates/takt/src/run.rs:521-553` | a grant written during a judgement step is "added" to the snapshot: refused, `remove_file`d; `Authority::read` → `None` after the run | `takt approve` while a run holds the case, the documented `--hours` unattended mode included; `approve` through the store also leaves an `approval` record naming a grant the case no longer holds | CONFIRMED | introduced | warning |
| F-E | `crates/takt/src/run.rs:104` (`path.is_dir()` follows links), `:154` (`remove_file` through them) | `keep.txt` in a directory outside the tree deleted via `<case>/link`; the refusal names `link/keep.txt`; the link itself stays | nothing found: `ClaudeCode` has `Read,Edit,Write,Glob,Grep,MultiEdit` and no Bash (`harness.rs:163-166`), so it cannot create a link; another harness could | INFEASIBLE | introduced | warning |
| F-F | `crates/takt/src/case.rs:366` (`is_finished(&dir)?` in `resumable`) | an empty `decisions/0001.json` in one case → `open_next` errors `EOF while parsing a value`; every bare run on the repository | nothing shown: a `std::fs::write` cut short by a crash; not reproduced | INFEASIBLE | introduced | note |
| F-G | `crates/takt/src/run.rs:539` (`starts_with(CASES_DIR)`), `git.rs:77,98` | `.engineering/cases-smuggled.txt` left out of the step's change set (string prefix), so the `tests.write` refusal did not see it, and committed by `commit_all` (the pathspec exclude matches the directory only); the run printed `committed 1 path(s)` and `ls-tree HEAD` lists it | nothing found: a harness naming a path with that prefix. Base has the same lines (`git show feed6b7:crates/takt/src/run.rs:397`, `git.rs:77,98`) | INFEASIBLE | pre-existing | note |
| F-H1 | `crates/takt/src/run.rs:542` | MI (drop `discard_all` on a case-directory refusal) survives the suite as handed | my case 8 catches it; the code is right, the line was untested | CONFIRMED | introduced | note |
| F-H2 | `crates/takt/src/validator.rs:886` | MN (record `merged` without the read-back) survives the suite as handed and `tests/forge.rs` | my case 9 catches it; the `Forge::merge` doc ("the merge itself answers nothing a validator records") had no test | CONFIRMED | introduced | note |

Fixes named, not applied. F-A/F-B: one cause, `seq` per run; number a resumed run's evaluations after the files on disk (as `written` does for evidence file names) or put a run id in decision names and record ids; `latest_decision` then stays by name. F-C: snapshot `tree.join(CASES_DIR)` rather than `self.case.dir`. F-D: `takt approve` refuses while a run holds the case (a marker the run writes at start and removes at stop), or the snapshot design is replaced by the store cross-check pass 1 named as option 3. F-E: `symlink_metadata` in `walk`, treat a link as a changed path and remove the link, never walk through it. F-F: an unreadable latest decision counts as not finished, printed on `out`. F-G: `p == CASES_DIR || p.starts_with(".engineering/cases/")`.

Pass-1 findings re-tested: F1 closed for `<case>/` (its red case green; MA/MF/MG caught), the class reopened one directory over (F-C) and with F-D as the cost. F2 closed on the gate-only flow, not on the story's flow (F-A). F3 closed as measured (claims unknown at the new revision; MC caught), its follow-through errors (F-B). F4 closed (MD caught). F5 closed (ME caught). F6 and F10: the coordinator's; `aep plan artifact show` revision 13 still carries the Ambiguities line on `denied`. F7: unchanged by decision. F8, F9: catchers still catch (M5, M6).

## 5. Attacked and could not break

- `grant`: a whitespace-padded capability is refused as not an identifier (`case.rs:251-257`); a case-different name is refused as not required (exact compare, `:262`); several required capabilities are collected, sorted and deduped (`:223-235`) — code read.
- `NoHarness` with two pending approvals: cannot occur, `context.approvals` is filtered to the one gate (`run.rs:497-501`); with judgement actions offered beside the pending gate it requests the gate (CLI case green).
- Step that errors: the case directory is restored before `stepped?` (`run.rs:520-525`); a harness that writes into `<case>/` and a source file in one step: both discarded, tree clean, HEAD unmoved (case 8 green).
- Resume: detached HEAD or another branch → the case branch is checked out (`case.rs:568`, pass-1 case green); a force-reset HEAD → revision = HEAD regardless; `pushed` ≠ revision → `ci.read` reads nothing and `pr.open` pushes again and reads the existing pull request at its new head (case 10's `push`, `view #1` before the error).
- An empty directory added under `<case>/`: not seen by the snapshot (`run.rs:104-112` records files only), lingers, harmless to the evaluation — code read; a mode-only change: not seen (bytes compared) — code read; `<case>/` replaced by a symlink: `is_dir` follows, same mechanism as F-E — code read, not run.
- A file beside the cases, `.engineering/cases/note.txt`: left out of the change set, never committed, exempt from the clean, skipped by `resumable` (no `case.yaml`) — code read; a planted file under another case's directory is removed by nothing (case 3's trace: read by the next run).
- First-commit surfaces, all green on re-run: the two-run acceptance with the double, `takt approve` and its `approval` record on the fixture store, resume by story and by id, the merge argv and request body, `merge_observation` numbered `0005-`, head moved and closed pull request, `--case` with `--intent`, `--harness none` on the grant-only case.
- A very large planted file: not measured.

## 6. Paths written outside the worktree

All under the unit's scratch `adversary-2/` (1.8M after cleanup): `append-lib.rs`, `append-lib-2.rs`, `mutants.py`, `mutants-summary.log`, `mutant-build.log`, `mutant/` (the source copy, its `target/` removed by me), `red-1-finished-on-resume.log`, `red-2-reopened.log`, `red-3-sibling.log`, `red-4-meanwhile.log`, `red-5-symlink.log`, `red-6-unreadable.log`, `red-7-smuggle.log`, `case-8-clean.log`, `case-9-still-open.log`, `case-10-re-established.log`, `suite-takt.log`, `suite-cli.log`, `suite-takt-final.log`, `suite-cli-final.log`, `gate-fmt.log`, `gate-clippy.log`, `gate-fmt-final.log`, `gate-clippy-final.log`, `mutant-M{A,B,C,D,E,F,G,I,J,N,5,6}-*.log`. Builds used `RUSTC_WRAPPER=sccache` (shared cache). Test scratch trees under the worktree's `target/tmp/` were removed by each case's `Scratch` drop (0 left). The harness wrote its own two background-task output files under its session directory. Lease `takt-w5-merge-adv2` released.

## 7. Findings

```findings
- file: crates/takt/src/case.rs
  line: 320
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Run::run numbers decisions from 1 on every run, so a resumed run overwrites the earlier run's files and latest_decision answers whichever run wrote more: after the story's own two-run acceptance the finished case reads open and is resumed by every later bare run (adv2_a_case_finished_by_a_resumed_run_is_not_resumed_by_the_next_run), and an open case can read finished (adv2_a_finished_case_resumed_at_a_new_commit_is_open_again)."
- file: crates/takt/src/run.rs
  line: 393
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A case resumed at a new revision re-runs its validators with seq restarted at 1, so pr.open writes pull-request-1 a second time and the next evaluation errors \"evidence `pull-request-1` is given more than once\", the duplicate left on disk (adv2_a_case_resumed_at_a_new_commit_re_establishes_its_claims_before_the_merge)."
- file: crates/takt/src/run.rs
  line: 519
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "The snapshot covers the running case's directory only, so a harness of a second case under the same tree writes a grant into the waiting case's authority.yaml unrefused, and that case's next run merges under it (adv2_a_grant_the_harness_writes_into_another_case_does_not_merge_that_case)."
- file: crates/takt/src/run.rs
  line: 521
  category: concurrency
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "A grant written while a judgement step is in flight is refused as the step's write and removed by the restore, so takt approve during a run loses the operator's decision (adv2_a_grant_given_while_a_judgement_step_runs_is_kept)."
- file: crates/takt/src/run.rs
  line: 104
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "snapshot follows a symbolic link planted under the case directory and restore deletes the files it found there, outside the tree (adv2_the_restore_does_not_delete_through_a_symlink_the_harness_planted); the shipped harness has no tool that creates a link."
- file: crates/takt/src/case.rs
  line: 366
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "One decision file that does not read makes resumable, and so every bare run on the repository, an error (adv2_a_decision_that_does_not_read_does_not_fail_the_frontier); a write cut short was not shown to happen."
- file: crates/takt/src/run.rs
  line: 539
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: "The cases-directory exemption is a string prefix, so .engineering/cases-smuggled.txt is invisible to the tests.write refusal yet committed by commit_all (adv2_tests_write_does_not_commit_a_non_test_path_beside_the_cases_directory); the same lines are at the base."
- file: crates/takt/src/run.rs
  line: 542
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Dropping discard_all on a case-directory refusal survives the suite as handed; adv2_a_refused_case_write_discards_the_tree_change_with_it catches it."
- file: crates/takt/src/validator.rs
  line: 886
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Recording merged without the forge's read-back after the merge survives the suite as handed and tests/forge.rs; adv2_pr_merged_records_nothing_when_the_forge_still_reports_the_pull_request_open catches it."
```
