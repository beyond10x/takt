---
format: aep.planning-md/3
id: review-result:adversary-protocol-pass-2
kind: review-result
status: active
title: Adversary, story-delivery-protocol, pass 2
relations:
- reviews: story:story-delivery-protocol
revision: 1
---
```
unit: story:story-delivery-protocol, pass 2 — branch impl/story-delivery-protocol at 467a6ab (base 1638755a), worktree takt-w1-protocol with my 4 cases uncommitted
verdict: CONFIRMED
cases: executed 9→13, red 0 (2 of the 4 are red against mutants the 9 cases miss; 0 red against the tree)
origin: introduced 1 / pre-existing 1 / undecided 0
wrote-outside-worktree: 3 (one directory tree, ~617M, listed in part 6)
needs-coordinator: the acceptance text the correction brief calls "corrected" is in no store I can read; the README's relative `--protocol` path (note); 18 pre-fix scratch trees still under target/tmp (note)
```

### 1. `git --no-pager diff --stat`

```
 crates/takt/tests/story_delivery.rs | 194 ++++++++++++++++++++++++++++++++++++
 1 file changed, 194 insertions(+)
```

Only the test file. The diff is `mod adv2` appended at crates/takt/tests/story_delivery.rs:483-676.

### Answers to the four questions asked

| Question | Answer | Evidence |
|---|---|---|
| Any assertion dropped or relaxed? | No | `diff` of pass 1's tree-copy test file against HEAD: the decided-sequence case keeps its 3-element `assert_eq!` unchanged, only the fifth record kind changed (`pull_request`→`merge_observation`); the 4 green cases are byte-identical in their assertions; removals are only `risk` from `open`/`adv_open` and the `let _ = remove_dir_all` lines the guard replaces |
| README invocation opens a case for real? | Yes, from the repository root | `target/debug/takt run --repo <scratch crate> --intent x --protocol protocols/story-delivery/1.yaml --harness none --iterations 1` printed `case takt-20261018T031401Z on story.delivery/1 in …/scratch-crate; inputs none`, `step 1: tests.run {}`, `evidence: test_result pass`, `stopped: Iterations (1); steps 1, tokens 0`, exit 3 (log: cli-root.log) |
| Guard removes the tree on panic and on success? | Yes, both | `adv2_a_passing_case_leaves_no_scratch_tree` and `adv2_a_failing_case_leaves_no_scratch_tree` green; after the 13-case suite plus 3 red mutant runs, `find target/tmp -newer crates/takt/tests/story_delivery.rs` lists nothing |
| New mutants the 9 cases miss? | Two, both reach the CLI loop | E: `tests.run` renamed; F: `abandoned` requires a claim. 9/9 green on each; my cases red (part 2) |

### 2. Cases added (all in `mod adv2`, file above)

| Case | Line | Asserts | Now | Against mutant |
|---|---|---|---|---|
| `adv2_a_passing_case_leaves_no_scratch_tree` | 568 | a case opened in the tree, then dropped: path gone | green | n/a (guard works) |
| `adv2_a_failing_case_leaves_no_scratch_tree` | 584 | a case that panics after opening a case: path gone | green | n/a (guard works) |
| `adv2_the_loop_runs_the_tests_validator_first_on_this_protocol` | 608 | `Run::run` with a `tests.run` validator and an idle harness, `iterations: 1`: `Stop::Iterations(1)`, 1 step, `tests.run {}` and `test_result pass` in the output, `tests.pass` true, no outcome legitimate | green | **red** on E and on F |
| `adv2_abandoned_needs_an_explicit_decision_not_a_claim` | 644 | `abandoned` is `blocked` at seq 0 and after each of the 5 records; `legitimate() == ["accepted"]` at the end | green | **red** on F |

Alone-runs against the tree (one each, before the suite): `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out` for all four, exit 0 (logs `alone-*.log`).

Red output, verbatim, case alone with `STORY_DELIVERY_PROTOCOL=` pointing at the mutant:

Mutant E (`protocols/story-delivery/1.yaml:106` `tests.run:` → `test.run:`), `red-E-loop.log`:
```
test adv2::adv2_the_loop_runs_the_tests_validator_first_on_this_protocol ... FAILED
thread 'adv2::adv2_the_loop_runs_the_tests_validator_first_on_this_protocol' (644095) panicked at crates/takt/tests/story_delivery.rs:621:9:
assertion `left == right` failed: case adv2-loop on story.delivery/1 in .../target/tmp/story-delivery-644094-adv2-loop; inputs none
  left: Err("a judgement step was offered [\"ci.read\", \"gate.run\", \"implementation.edit\", \"repository.inspect\", \"test.run\", \"tests.write\"] before any validator ran")
 right: Ok(Iterations(1))
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.01s
```
Exit 101. The nine original cases on E (tree copy, `original-suite-on-E-tests-run-renamed.log`): `test result: ok. 9 passed; 0 failed`.

Mutant F (`:155` `decision: explicitly_abandoned` → `claim: tests.pass`), `red-F-abandoned.log`:
```
test adv2::adv2_abandoned_needs_an_explicit_decision_not_a_claim ... FAILED
thread '...' (644144) panicked at crates/takt/tests/story_delivery.rs:661:13:
assertion `left == right` failed: after test_result pass
  left: "legitimate"
 right: "blocked"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.01s
```
Exit 101. Same mutant, the loop case (`red-F-loop.log`): `left: Ok(Completed("abandoned"))  right: Ok(Iterations(1))`, meaning the CLI would exit 0 with `stopped: Completed (abandoned)` after one passing test run. The nine original cases on F: `test result: ok. 9 passed; 0 failed`.

### 3. Suite run (after part 2 existed)

```
cargo test -p b10x-takt --test story_delivery
     Running tests/story_delivery.rs (target/debug/deps/story_delivery-5f342fc041dcfb44)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```
Exit 0. The `before` number: `cargo test -p b10x-takt --test story_delivery -- --skip adv2_` → `9 passed; 0 failed; 4 filtered out`, matching the implementing state's "9 passed, 0 failed" in commit 467a6ab. Green suite: the correction holds against the tree; the two mutant killers are the value added.

### 4. Judgement findings (tree at 467a6ab)

| # | file:line | Verdict | Origin | Finding | What reaches it |
|---|---|---|---|---|---|
| 1 | `.engineering/planning/story/story-delivery-protocol.md:57` (worktree store, revision 5; identical at base and HEAD) | INFEASIBLE | pre-existing | The recorded Acceptance still names the fifth record `pull_request` merged; the protocol, the ladder and the decided-sequence case all use `merge_observation`. The correction brief says "the store has since been corrected"; neither this tree's store nor the main tree (revision 3) carries it. Cannot be fixed in this unit (store is the coordinator's) | anyone judging the unit by its recorded acceptance |
| 2 | `README.md:25` | CONFIRMED | introduced | `--protocol protocols/story-delivery/1.yaml` resolves against the cwd only (`run.rs:103`: `is_file()` else built-in lookup); from any other directory the run fails with `takt: the major of `protocols/story-delivery/1.yaml` is not a number: invalid digit found in string`, exit 1 (`cli-wrong-cwd.log`). README:12 advertises `--repo <path>` with no cwd constraint. Fix is a doc phrase ("from this repository's root") or an absolute path; the resolution code is pre-existing and not this unit's | README:12's pattern followed from the target repository |

Residue, not raised as findings: (a) `scratch()` at `story_delivery.rs:67-72` keys stale removal on `std::process::id()`, so the 18 trees left by runs before the fix (04:40–05:01, 4.2M under `target/tmp`) are never reclaimed by a later run; they sit in the build directory, which I do not touch. (b) The protocol's `obligations` section is not load-bearing for any outcome (Canon 0.6.0 `eval/outcomes.rs:118` decides an outcome from its `requires` over claim values alone); a mutant deleting it stays green and changes only the harness prompt and the `NoProgress` state; the story never asked for obligations, so not a defect.

### 5. Attacked and could not break

- Every assertion of pass 1's six cases survives the correction unchanged (diff above).
- The guard: success path, panic path, and the existing unremovable-tree case; no-op or error-swallowing Drop mutants are already caught by `a_scratch_tree_that_cannot_be_removed_fails_the_case_loudly`.
- `CaseDir::open` with no inputs for this protocol, through both `CaseDir::open` and `Run::open`, and through the built binary.
- Removal of `risk`: no case or doc still references it (grep on the tree).
- Panic strategy: the workspace sets no `panic = "abort"`, so unwinding reaches the guard.

### 6. Paths written outside the worktree

All under the unit's scratch `adversary-2/`, total about 617M:
- `tree-copy/` — fresh rsync of the worktree without `target/` and `.git/` (692K), its own `target/` **608M**; its protocol file restored to the tree's after the mutant runs
- `scratch-crate/` — a one-test cargo crate as a git repo for the CLI probe; the run wrote `.engineering/cases/takt-20261018T031401Z/`, `Cargo.lock` and a `target/` (7.5M) into it
- `adv2.rs`, `mutants/E-tests-run-renamed.yaml`, `mutants/F-abandoned-on-a-claim.yaml`, and the logs `alone-*.log`, `red-E-loop.log`, `red-F-abandoned.log`, `red-F-loop.log`, `original-suite-on-{E,F}*.log`, `cli-root.log`, `cli-wrong-cwd.log`, `suite-after.log`, `suite-without-adv2.log`

Lease `takt-w1-protocol-adv2` released. No store command run, no commit, no implementation or protocol file touched.

### 7. Findings block

```findings
- file: .engineering/planning/story/story-delivery-protocol.md
  line: 57
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: pre-existing
  message: "The recorded Acceptance names the fifth record `pull_request` merged while the protocol, the ladder and the decided-sequence case use `merge_observation` merged; the corrected text the correction brief refers to is in neither this tree's store (revision 5) nor the main tree's (revision 3), and this unit may not write the store."
- file: README.md
  line: 25
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The advertised `--protocol protocols/story-delivery/1.yaml` resolves only from the repository root; from the target repository's directory the binary exits 1 with `the major of `protocols/story-delivery/1.yaml` is not a number`, while README:12 advertises `--repo <path>` with no cwd constraint."
```
