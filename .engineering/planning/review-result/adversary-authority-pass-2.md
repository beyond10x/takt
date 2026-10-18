---
format: aep.planning-md/3
id: review-result:adversary-authority-pass-2
kind: review-result
status: active
title: Adversary, authority-hardening, pass 2
relations:
- reviews: story:authority-hardening
revision: 1
---
unit: story:authority-hardening at 36f8405 (base ae94ccb), worktree w6-authority-hardening, plus one untracked test file I added
verdict: CONFIRMED (one warning, pre-existing) and NEEDS-CHANGE (one note, introduced)
cases: executed 139→142, red 2
origin: introduced 2 / pre-existing 1 / undecided 0
wrote-outside-worktree: 5 paths (part 6)
needs-coordinator: yes. The sibling-grant revert happens in `run.rs` 566-578, which this unit does not own (only `is_due` and `Run::run`). Route it to its own story or to the owner of that range.

**1. Diff stat**

`git --no-pager diff --stat` is empty because no tracked file changed. `git status --short` shows one path: `?? crates/takt/tests/authority_adv2.rs`. It is a test file. `cargo fmt -p b10x-takt` reformatted only that file.

**2. Cases added** (`crates/takt/tests/authority_adv2.rs`)

| case | what it asserts | now |
|---|---|---|
| `adv2_a_grant_on_a_sibling_case_given_during_another_case_s_judgement_step_is_kept` | Case B waits at its gate in the same tree. During a judgement step of a run on case A, the operator grants B and is told it worked. After the run, B still holds that grant. | red |
| `adv2_has_evidence_answers_nothing_while_a_record_does_not_read` | One record matches and a broken record sits after it. `has_evidence` must return an error, as its doc says. | red |
| `adv2_probe_edge_case_ids_open_one_directory_or_leave_nothing` | 12 edge ids. Each one either opens exactly one directory under the cases dir, or is refused and leaves no directory, no branch and HEAD on main. | green |

Red output, from running `authority_adv2` alone (exit 101):
```
the operator was told b is granted at e5de320e…, and b holds no grant after the run on a: Authority { decisions: [] }; run on a: Ok(StepRefused("ci.read wrote into the cases directory: .engineering/cases/b/authority.yaml"))
a record that does not read (9999-broken.json) beside a matching one was an answer: Ok(true)
```
- Opened: `fix-über`, `CON`, `nul`, `-rf`, a 250-byte id, `rtl-\u{202e}txt.exe`, `ls\u{2028}ep`.
- Refused cleanly: `x.`, `x.lock`, `C:evil`, `a:b`, a 300-byte id.

**3. Suite, run after the cases existed**

| step | exit | result |
|---|---|---|
| `cargo fmt -p b10x-takt --check` | 1 → 0 | failed on my new file; passed after `cargo fmt -p b10x-takt` |
| `cargo clippy -p b10x-takt --all-targets --locked -- -D warnings` | 0 | clean |
| `cargo test -p b10x-takt --locked --no-fail-fast` | 101 | `authority_adv2`: `FAILED. 1 passed; 2 failed`; every other binary ok (lib 14, authority 11, authority_adv1 3, forge 35, frontier 15, gate_validator 16, loop 2, merge_under_authority 30, story_delivery 13) |

The 139 "before" is the sum of those other binaries in the same run. It matches the 139 that correction 1 reported.

**4. Findings**

| file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|
| run.rs:573-577 | Before a judgement step the run snapshots the whole cases directory and puts it back afterwards. So a `takt approve` on a sibling case during that step is reported as granted, then erased. The run on A is then refused and blamed on its harness, and `discard_all` throws away A's work. The hold covers one case; the snapshot covers every case in the tree. | CONFIRMED / pre-existing (the snapshot and restore are the same at ae94ccb) | Two cases in one tree, for example `takt run --case A` while B waits for its grant, and an approve during a step that takes minutes. The default `takt run` resumes the one active case, so this needs `--case` or an intent run. Probably also lost (inferred, not run): the story's approval record, which `git checkout -- .` discards. |
| case.rs:1075 | `has_evidence` returns `Ok(true)` at the first matching record and never reads the records after it. Its doc says "a record that does not read is an error … never an answer". | NEEDS-CHANGE / introduced | Nothing reaches it in a run: `evaluate` reads every record first, and `is_due` swallows the error anyway (`unwrap_or(true)`, known). Fix: read every record before answering, or narrow the doc. |
| case.rs:56-67 | `check_id` accepts Unicode format and separator characters such as U+202E and U+2028. They show up deceptively in messages and in the `takt/<id>` branch name. | INFEASIBLE / introduced | Only an id the operator types, or one derived from a store id. No path escape on Linux. |

**5. Attacked, could not break**
- **Correction F1 holds:** `evaluate` filters the grant by the revision in the same `case.yaml` bytes that Canon reads.
- **Merge at an in-memory revision different from disk:** `PrMerged` still requires that pushed, the PR head and `grants_at` all equal the revision it is about to merge. Whatever gets merged was granted.
- **Correction F2 holds:** pass 1's empty-record case is green.
- **`PrMerged` checks:** the grant check runs only after the pushed and PR-head equality checks. Removing it is caught by the existing `pr_merged_merges_nothing_under_a_grant_for_another_revision`.
- **Approve against a held case:** a run or `resume` holding the case refuses the approve. A refused grant releases its lock on drop. `hold()` failing in `Run::run` returns without releasing.
- **Edge case ids:** trailing dots, `.lock`, drive syntax and over-long ids are refused by git before anything is written. The rest stay one directory. Windows paths are not reached because CI is ubuntu only.
- **Store record vs grant:** record first, then write. A failed write leaves a record without a grant and says so in the error, which the story accepts.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/authority-hardening/adv2/` (directory)
- `…/adv2/red-alone.log`
- `…/adv2/fmt.log`
- `…/adv2/clippy.log`
- `…/adv2/suite.log`

The test scratch trees went under the worktree's `target/tmp` and were removed on drop. The worktree lease I took for this pass is released. No processes are left running.

```findings
- file: crates/takt/src/run.rs
  line: 573
  category: concurrency
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: the judgement-step snapshot and restore covers every case in the tree while the hold covers one, so a takt approve on a sibling case during another case's step is reported as granted, then reverted, and the step is refused as the harness's write
- file: crates/takt/src/case.rs
  line: 1075
  category: contract-drift
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: has_evidence answers Ok(true) at the first matching record without reading later ones, although its doc promises that a record that does not read is an error and never an answer
- file: crates/takt/src/case.rs
  line: 67
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: check_id refuses only Unicode control characters, so ids carrying U+202E or U+2028 open cases and branches whose names display deceptively
```
