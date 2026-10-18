---
format: aep.planning-md/3
id: review-result:adversary-gate-pass-1
kind: review-result
status: active
title: Adversary, gate-validator, pass 1
relations:
- reviews: story:gate-validator
revision: 1
---
```
unit: story:gate-validator at 77beec1db18358a4b76c487849683bd7a2de458d (branch impl/gate-validator, base 4dcbfae0), working tree takt-w3-gate
verdict: CONFIRMED (3 red cases; strongest: an unknown gate at HEAD stops the run before the harness ever gets a step)
cases: executed 15→26, red 3 (gate_validator 3→11 red 2; takt-cli package 12→15 red 1; "before" = the same runs with my 8 + 3 cases subtracted, no pre-emptive run)
origin: introduced 10 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, the unit's scratch adversary-1/ (7 entries; mutant/target 825M)
needs-coordinator: severity call on finding 1 (NoProgress before the first judgement step was the brief's intent; its cost is measured below) and whether the project-file gap (finding 2) is this unit's or a follow-up story
```

## 1. Diff (test paths only)

```
 crates/takt/tests/gate_validator.rs | 440 ++++++++++++++++++++++++++++++++++++
 1 file changed, 440 insertions(+)
?? crates/takt-cli/tests/gate_cli.rs
```

`cargo fmt -p b10x-takt -p b10x-takt-cli --check` exit 0 and `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` clean with my files, so the unit's gate is not disturbed by formatting.

## 2. Cases added (written first; each red case run alone before any suite)

`crates/takt/tests/gate_validator.rs` (appended after the unit's three cases):

| Case | Asserts | Now |
|---|---|---|
| `with_two_gates_and_the_second_false_the_result_is_fail_and_the_summary_names_it` | one `fail` record, summary contains `gate license false: fs.file.exists("LICENSE")`, not `gate manifest` | green |
| `an_unknown_gate_beside_a_false_one_is_fail_and_the_summary_names_the_false_gate` | Kleene: unknown ∧ false → `fail`; the false gate is named, not the unknown one | green |
| `an_empty_gates_list_aborts_the_run_and_writes_no_record` | `gates: []` → `run()` is `Err` naming the document and "a gates document cannot be empty", 0 records, `gate.green` unknown | green |
| `an_unregistered_function_aborts_the_run_rather_than_passing` | `no.such.function(...)` → `Err`, 0 records | green |
| `an_unknown_gate_stops_the_run_as_no_progress_after_one_step` | budget 10: `Stop::NoProgress`, `steps == 1`, one `step` line | green (red under mutant M3) |
| `an_unknown_gate_does_not_starve_the_judgement_step_that_would_make_it_observable` | harness offered `implementation.edit` once, then `accepted` with one `pass` at the new HEAD | **red** |
| `after_a_commit_the_gate_runs_again_at_the_new_head_and_a_stale_record_does_not_count` | `fail`@HEAD0 id `gate-result-1`, `pass`@HEAD1 id `gate-result-3`, producer `takt.gate-run`, 3 steps, `accepted` | green |
| `a_command_bound_gate_with_its_project_file_is_evaluated` | `process.succeeded("check")` with `.engineering/assertions-project.yaml` admitting `check.sh` → `accepted` | **red** |

`crates/takt-cli/tests/gate_cli.rs` (new, spawns the binary):

| Case | Asserts | Now |
|---|---|---|
| `an_absolute_gates_path_inside_the_repository_evaluates_the_gate` | `--gates /abs/repo/.engineering/gates.yaml` → exit 0, `gate_result pass` | **red** |
| `a_gates_document_outside_the_repository_is_refused_and_never_passes` | absolute outside and `../gates.yaml` → exit ≠ 0, no `gate_result pass` | green |
| `a_gates_path_that_names_a_directory_registers_no_validator` | `--gates .engineering` → exit 1, "no harness: a judgement step among gate.run is left" | green (red under M5) |

Red output, each case alone:

```
---- an_unknown_gate_does_not_starve_the_judgement_step_that_would_make_it_observable stdout ----
thread '...' panicked at crates/takt/tests/gate_validator.rs:589:5:
assertion `left == right` failed: the harness is offered implementation.edit once while the gate is unknown at HEAD; the run stopped NoProgress after 1 step(s):
case starved on gate.edit/1 in .../target/tmp/gate-validator-3373755-starved; inputs none
frontier: gate.run (admissible), implementation.edit (admissible)
step 1: gate.run {}
  effect: the gates document .engineering/gates.yaml: unknown; gate readme unknown: text.contains("README.md", "ready") (sha256:...: No such file or directory (os error 2)); no record, the claim stays unknown
frontier: gate.run (admissible), implementation.edit (admissible)
  left: 0
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 2.09s

---- a_command_bound_gate_with_its_project_file_is_evaluated stdout ----
thread '...' panicked at crates/takt/tests/gate_validator.rs:684:14:
the run stops: the gate is a command the project file admits: prepare the gates document .engineering/gates.yaml
Caused by:
    command binding check is not registered
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 2.08s

---- an_absolute_gates_path_inside_the_repository_evaluates_the_gate stdout ----
thread '...' panicked at crates/takt-cli/tests/gate_cli.rs:116:5:
assertion `left == right` failed: stdout case absolute on gate.only/1 in .../gate-cli-3376191-absolute/repo; inputs none
frontier: gate.run (admissible)
 stderr takt: prepare the gates document /.../gate-cli-3376191-absolute/repo/.engineering/gates.yaml: input path must stay below source root
  left: 1
 right: 0
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 2.09s
```

## 3. Suite runs (after part 2 existed)

`cargo test -p b10x-takt --test gate_validator`:

```
running 11 tests
… 9 ok …
test an_unknown_gate_does_not_starve_the_judgement_step_that_would_make_it_observable ... FAILED
test a_command_bound_gate_with_its_project_file_is_evaluated ... FAILED
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.41s
```

`cargo test -p b10x-takt-cli`:

```
Running unittests src/main.rs:      test result: ok. 6 passed; 0 failed; ... finished in 2.16s
Running tests/frontier_cli.rs:      test result: ok. 6 passed; 0 failed; ... finished in 0.12s
Running tests/gate_cli.rs:
test a_gates_path_that_names_a_directory_registers_no_validator ... ok
test an_absolute_gates_path_inside_the_repository_evaluates_the_gate ... FAILED
test a_gates_document_outside_the_repository_is_refused_and_never_passes ... ok
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.99s
```

Exit status of both: non-zero.

Mutants, run on the scratch copy (worktree untouched; pristine restored and diffed clean after):

| Mutant | Unit's suite | My cases |
|---|---|---|
| M1 `validator.rs` Unknown → `Some("pass")` | caught (`an_assertion_the_provider_cannot_observe_writes_no_record`) | also caught |
| M2 `validator.rs` False → `Some("pass")` | caught (`a_false_assertion_writes_one_fail_record...`) | also caught |
| M3 `run.rs:285` line removed (loop change reverted) | **survives**: 3/3 green | caught: `Iterations(10)` vs `NoProgress`, 19.5 s run |
| M4 `validator.rs:166` `.find(truth == report.truth)` → `.next()` | **survives** | caught by both two-gate cases (summary names `manifest`/`readme`) |
| M5 `main.rs:179` `is_file()` → `exists()` | **survives**: 12/12 green | caught by the directory case |

## 4. Findings (commit 77beec1, working tree above)

| # | file:line | What was measured | What reaches it | Verdict | Origin | Sev |
|---|---|---|---|---|---|---|
| 1 | `crates/takt/src/run.rs:285` (with the due rule `:258-264`) | gate unknown at HEAD → validator due every pass; with the new line the run stops `NoProgress` at pass 2, harness called 0 times (red case `:589`); with the line removed (M3) it spins to the budget, harness still 0 times | any gates document with an assertion unavailable at the first revision: the guide's own example `text.contains("README.md", "Apache")` on a repository without README.md; `story-delivery/1` admits `gate.run` with no precondition, so the stop lands at step 2 before any edit. Fix named, not applied: remember (validator, revision) observed-without-record so it is not due again at that revision, or write a result-less `gate_result` at HEAD (canon-evidence/1 admits `result: None`; `has_evidence` sees it, `gate.green` does not match it) | CONFIRMED | introduced | warning |
| 2 | `crates/takt/src/validator.rs:152` | `prepare(tree, gates, None, None)`: no project file, so `process.succeeded("check")` aborts with "command binding check is not registered" even with `.engineering/assertions-project.yaml` admitting it (red case `:684`; the path is my assumption, the validator has no seam at all) | every command-bound gate: `process.succeeded`, `tests.run`, `proj.tests.run($scope)`, the design's example `tests.unit.failed == 0` (`docs/design/takt.md:59`). Fix: a project path on `GateRun` / `--project`, passed to `prepare` | CONFIRMED | introduced | warning |
| 3 | `crates/takt-cli/src/main.rs:179-180` | absolute `--gates` to the document inside the repository is registered, then aborts at the first observation, exit 1, "input path must stay below source root" (it is below the root) (red case `gate_cli.rs:116`) | a user passing `$PWD/.engineering/gates.yaml`. Fix: `strip_prefix(&repo)` or refuse non-relative at parse with a clear message | CONFIRMED | introduced | warning |
| 4 | `crates/takt/tests/gate_validator.rs:227` | the unit's unknown case asserts only `stop != Completed` with budget 2: it stays green under M3 while the loop runs the validator ten times | the suite is the only thing guarding the loop line this unit added; my `an_unknown_gate_stops_the_run_as_no_progress_after_one_step` is the rewrite (green now, red under M3) | CONFIRMED | introduced | warning |
| 5 | `crates/takt/src/validator.rs:160-166` | M4 (name the first assertion, not the first sharing the report's truth) survives the unit's suite; only single-gate documents are tested | any multi-gate document; my two-gate cases catch it | CONFIRMED | introduced | note |
| 6 | `crates/takt-cli/src/main.rs:179` | M5 (`exists()`) survives the unit's suite; my directory case catches it | `--gates` naming a directory | CONFIRMED | introduced | note |
| 7 | `crates/takt-cli/src/main.rs:179-181` | registration decided once before the run; a gates document the harness creates mid-run is never evaluated: `gate.run` stays a judgement step the harness can only answer with no file change → `NoProgress` | a story whose work is to add the gates document. Not constructible from the CLI (`--harness none` writes nothing) | INFEASIBLE | introduced | note |
| 8 | `crates/takt/src/validator.rs:212-224` | the `Bundle` (observations, `source_identity`, `context_identity`) is dropped; the record carries pass/fail at HEAD and nothing replayable; the story's Context promised "the retained evidence path as its reference" | every record. `canon-evidence/1` has no reference field (`evidence.rs:7-17`), so the fix is a retained bundle in the case directory | CONFIRMED | introduced | note |
| 9 | `crates/takt/src/run.rs:353-366` | `implementation.edit` admits any path, including `.engineering/gates.yaml`: the harness can weaken the gate it is measured by; the loop refuses only the inspect/tests.write shapes | every harness run under story-delivery/1 | INFEASIBLE (a loop/protocol decision, same exposure as tests) | introduced | note |
| 10 | `crates/takt/src/validator.rs:152-156` | one observation on a tiny tree: 1.94 s, 2.06 s of it `prepare`, `collect` 0.2 ms, `evaluate` 21 µs; SHA-256 of the 61 MB debug test binary alone 1.94 s (assertions `lib.rs:278` hashes `current_exe` every observation); +64 MiB in the tree → +4.2 s, so the tree is hashed twice per observation (`lib.rs:334-335` and `:492-498`), `.git`/`target` excluded, `.engineering/cases` not | every observation; debug build. The constant is the executable digest, not the tree; a release build shrinks it ~10× | INFEASIBLE (library behaviour) | introduced | note |

## 5. Attacked and not broken

- Two gates, one false, either order, and unknown-then-false: `fail`, the summary names the false gate.
- Invalid document (empty list, unregistered function): `observe` errors, the run aborts, zero records, never a pass; zero gates is refused by the library, so no vacuous pass.
- Record at a stale revision after a commit: `fail`@HEAD0 then `pass`@HEAD1, ids `gate-result-1`/`gate-result-3`, producer `takt.gate-run`, accepted.
- `--gates` outside the tree (absolute and `../`): refused by the library's input rule, exit 1, no pass; a directory: not registered; absent: judgement step (unit's own case).
- Mutants pass-on-unknown and pass-on-fail: the unit's own cases catch both.

## 6. Paths written outside the worktree

The unit's scratch `adversary-1/` — `gate_validator-additions.rs`, `probe-timing.rs`, `run-mutants.sh`, `validator.rs.pristine`, `run.rs.pristine`, `main.rs.pristine`, `mutant/` (1.2M source copy without `target/` and `.git`; its `mutant/target/` is 825M). Scratch trees my cases made under the worktree's `target/tmp/` were removed by their guards (verified empty). Lease `takt-w3-gate-adv1` acquired and released.

```findings
- file: crates/takt/src/run.rs
  line: 285
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "A gate unknown at HEAD is due every pass, so the run stops NoProgress at pass 2 (or spins without the new line) and the harness never gets the implementation.edit that would make the gate observable."
- file: crates/takt/src/validator.rs
  line: 152
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "prepare is called with no project file, so every command-bound gate (process.succeeded, tests.run, the design's tests.unit.failed == 0) aborts the run with 'command binding ... is not registered'."
- file: crates/takt-cli/src/main.rs
  line: 179
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "An absolute --gates path to the document inside the repository registers the validator and then aborts the run at the first observation with 'input path must stay below source root'."
- file: crates/takt/tests/gate_validator.rs
  line: 227
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "The unknown-gate case asserts only stop != Completed with a budget of 2, so it stays green when the loop line at run.rs:285 is removed and the validator runs every pass."
- file: crates/takt/src/validator.rs
  line: 160
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Naming the first assertion instead of the first sharing the report's truth survives the unit's suite; only single-gate documents are tested."
- file: crates/takt-cli/src/main.rs
  line: 179
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "is_file() replaced by exists() survives the unit's suite; a --gates path naming a directory is not tested."
- file: crates/takt-cli/src/main.rs
  line: 181
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "Registration is decided once before the run, so a gates document the harness creates mid-run is never evaluated and gate.run stays a judgement step the harness can only idle on."
- file: crates/takt/src/validator.rs
  line: 212
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The evaluated bundle with its source and context identities is dropped, so the record cannot be replayed and the story's promised retained-evidence reference does not exist."
- file: crates/takt/src/run.rs
  line: 353
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "implementation.edit admits a change to .engineering/gates.yaml, so the harness can weaken the gate it is measured by; the loop refuses nothing for that path."
- file: crates/takt/src/validator.rs
  line: 156
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "One observation costs 1.94 s on a tiny tree in a debug build, 1.94 s of it the SHA-256 of the 61 MB executable in prepare, and the tree is hashed twice per observation (+4.2 s for 64 MiB)."
```
