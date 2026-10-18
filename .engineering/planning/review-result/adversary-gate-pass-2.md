---
format: aep.planning-md/3
id: review-result:adversary-gate-pass-2
kind: review-result
status: active
title: Adversary, gate-validator, pass 2
relations:
- reviews: story:gate-validator
revision: 1
---
```
unit: story:gate-validator at 66f3b3ab7e1604eba4575083922f0ae60279e2b2 (77beec1 + correction 66f3b3a, branch impl/gate-validator, base 4dcbfae0), working tree takt-w3-gate
verdict: CONFIRMED (4 red cases; strongest: an explicit --project to a missing file is dropped and the gate's commands are admitted by the default project file the user never named)
cases: executed 73→78, red 4 (takt 56→59, takt-cli 17→19; "before" = the implementor's r1-final-test-*.log per-binary counts, no pre-emptive run)
origin: introduced 8 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, the assigned scratch adversary-2/ (41 entries; the mutant copy's target/ removed)
needs-coordinator: severity of finding 1 (today reachable only through the library API) and whether "--project given when it exists" (finding 4) is the contract the CLI keeps or a refusal replaces it
```

## 1. Diff (test paths only)

```
 crates/takt-cli/tests/gate_cli.rs   |  64 ++++++++
 crates/takt/tests/gate_validator.rs | 314 ++++++++++++++++++++++++++++++++++++
 2 files changed, 378 insertions(+)
```

`cargo fmt -p b10x-takt -p b10x-takt-cli --check` exit 0; `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` exit 0, both with my files. Lease `takt-w3-gate-adv2` acquired before the first edit and released.

## 2. Cases added (written first; each run alone before any suite)

`crates/takt/tests/gate_validator.rs` (appended after pass 1's block):

| Case (line) | Asserts | Now |
|---|---|---|
| `an_unknown_gate_does_not_stop_the_run_while_another_validator_is_due` (:849) | validators `[GateRun, TestsPass]` on a protocol whose two actions are both validator-owned; gate unknown at pass 1; one `test_result` record, 2 steps, `tests.pass` true | **red** |
| `a_record_does_not_land_when_its_bundle_cannot_be_retained` (:936) | harness writes LICENSE and makes the case's `artifacts/` read-only; the run errors naming `gate-result-3.bundle.json`, no `gate_result` at the new HEAD, `gate.green` unknown | **red** |
| `every_observation_retains_its_own_bundle_and_each_replays_to_its_truth` (:994) | fail@HEAD0 then pass@HEAD1: `artifacts/` holds exactly `gate-result-1.bundle.json` and `gate-result-3.bundle.json`; each deserialises as `Bundle` and `evaluate(&bundle, bundle.evaluated_at, None, None)` answers False / True | green (red under M5) |

`crates/takt-cli/tests/gate_cli.rs` (appended):

| Case (line) | Asserts | Now |
|---|---|---|
| `a_relative_gates_path_through_a_symlink_outside_the_repository_is_refused_before_the_run` (:274) | `--gates gates/link.yaml`, the link to a file outside: exit ≠ 0, stderr names `--gates` and "not under the repository", no `case ` line, no `.engineering/cases` | **red** |
| `an_explicit_project_path_that_does_not_exist_is_refused_not_replaced_by_the_default` (:304) | command gate, project file at the default path, `--project gates/nope.yaml`: exit ≠ 0, stderr names `--project` and the path, no `gate_result pass`, no case | **red** |

Red output, each case alone (`cargo test -p <pkg> --test <file> -- --exact <case>`):

```
---- an_unknown_gate_does_not_stop_the_run_while_another_validator_is_due stdout ----
thread '...' panicked at crates/takt/tests/gate_validator.rs:882:5:
assertion `left == right` failed: tests.run was admissible with no evidence at the second pass and never ran; the run stopped NoProgress after 1 step(s):
case gate-then-tests on gate.and.tests/1 in .../target/tmp/gate-validator-3924965-gate-then-tests; inputs none
frontier: gate.run (admissible), tests.run (admissible)
step 1: gate.run {}
  effect: the gates document .engineering/gates.yaml: unknown; gate readme unknown: text.contains("README.md", "x") (sha256:b5d9dd2e...: No such file or directory (os error 2)); no record, the claim stays unknown; retained as gate-result-1.bundle.json
  artifact: .engineering/cases/gate-then-tests/artifacts/gate-result-1.bundle.json
frontier: gate.run (admissible), tests.run (admissible)
  left: 0
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 2.00s
EXIT=101

---- a_record_does_not_land_when_its_bundle_cannot_be_retained stdout ----
thread '...' panicked at crates/takt/tests/gate_validator.rs:980:5:
a gate_result landed at ce99ec50c897c853ba3e6b7a4db08ea927bc8285 without its retained bundle: [Object {"format": String("canon-evidence/1"), "id": String("gate-result-3"), "kind": String("gate_result"), ..., "producer": String("takt.gate-run"), "result": String("pass"), "subject": String("implementation"), "subject_revision": String("ce99ec50c897c853ba3e6b7a4db08ea927bc8285")}]
...
step 3: gate.run {}
  effect: the gates document .engineering/gates.yaml: pass; every gate true: license; retained as gate-result-3.bundle.json
  evidence: gate_result pass
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 4.05s
EXIT=101

---- a_relative_gates_path_through_a_symlink_outside_the_repository_is_refused_before_the_run stdout ----
thread '...' panicked at crates/takt-cli/tests/gate_cli.rs:289:5:
stdout case symlink on gate.only/1 in .../target/tmp/gate-cli-3929749-symlink/repo; inputs none
frontier: gate.run (admissible)
 stderr takt: prepare the gates document gates/link.yaml: input symlink escapes source root
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1.98s
EXIT=101

---- an_explicit_project_path_that_does_not_exist_is_refused_not_replaced_by_the_default stdout ----
thread '...' panicked at crates/takt-cli/tests/gate_cli.rs:315:5:
assertion `left != right` failed: --project gates/nope.yaml does not exist and the run went on: stdout case project-missing on gate.only/1 in .../gate-cli-3930302-project-missing/repo; inputs none
frontier: gate.run (admissible)
step 1: gate.run {}
  effect: the gates document .engineering/gates.yaml: pass; every gate true: check; retained as gate-result-1.bundle.json
  evidence: gate_result pass
  artifact: .engineering/cases/project-missing/artifacts/gate-result-1.bundle.json
frontier: gate.run (admissible)
stopped: Completed (accepted); steps 1, tokens 0, case .../repo/.engineering/cases/project-missing
 stderr 
  left: 0
 right: 0
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 2.00s
EXIT=101
```

The green case alone: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 4.04s`, EXIT=0. `target/tmp/` empty after every run (the read-only tree was restored by its guard).

## 3. Suite runs (after part 2 existed)

`cargo test -p b10x-takt` stopped after `gate_validator` failed (fail-fast; `loop` and `story_delivery` not executed), so the count comes from `cargo test -p b10x-takt --no-fail-fast`:

```
Running unittests src/lib.rs        test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/frontier.rs           test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
Running tests/gate_validator.rs
test an_unknown_gate_does_not_stop_the_run_while_another_validator_is_due ... FAILED
test a_record_does_not_land_when_its_bundle_cannot_be_retained ... FAILED
test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.25s
Running tests/loop.rs               test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/story_delivery.rs     test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Doc-tests takt                      test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=101
```

`cargo test -p b10x-takt-cli`:

```
Running unittests src/main.rs       test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.17s
Running tests/frontier_cli.rs       test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/gate_cli.rs
test an_explicit_project_path_that_does_not_exist_is_refused_not_replaced_by_the_default ... FAILED
test a_relative_gates_path_through_a_symlink_outside_the_repository_is_refused_before_the_run ... FAILED
test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.64s
EXIT=101
```

Mutants, on the scratch copy (`cp` without `target/` and `.git/`, built offline there; the three pristine files restored and `diff -q` clean after; the copy's 934M `target/` removed). "Caught by" lists cases other than my four always-red ones:

| Mutant | Change | Caught by |
|---|---|---|
| M1 | `run.rs:276-278` set guard removed | `an_unknown_gate_does_not_starve…` (pass 1) |
| M2 | `run.rs:317` → `last_idle = None` | `an_unknown_gate_stops_the_run_as_no_progress_after_one_step` (pass 1) |
| M3 | `run.rs:317` → `last_idle = Some(state)` | `an_unknown_gate_does_not_starve…` |
| M9 | `run.rs:315` `is_empty()` inverted | both pass-1 cases above |
| M4 | `validator.rs:191-195` default-project fallback removed | `a_command_bound_gate_with_its_project_file_is_evaluated` (pass 1) |
| M5 | `validator.rs:236` artifact name → constant `gate-result-1.bundle.json` | **only my** `every_observation_retains_its_own_bundle…`; the unit's `the_evaluated_bundle_is_retained…` stays green |
| M10 | `validator.rs:218` `.find(truth == report.truth)` → first assertion | the two pass-1 two-gate cases |
| M6 | `main.rs:243` canonicalize failure → error | **survives** (no case gives an absolute path that does not exist) |
| M7 | `main.rs:186` gates `is_file()` → `exists()` | `a_gates_path_that_names_a_directory…` (pass 1) |
| M8 | `main.rs:188` project `is_file()` → `exists()` | **survives** (no case gives `--project` a directory) |

## 4. Findings (commit 66f3b3a, working tree above)

| # | file:line | Measured | Reaches it | Verdict | Origin | Sev |
|---|---|---|---|---|---|---|
| 1 | `crates/takt/src/run.rs:317` (with the NoProgress check at `:250`) | gate unknown, `offered` empty → `last_idle = Some(state)`; the next pass returns `NoProgress` at `:250` before `due` is computed, so the second validator, admissible with no evidence, never runs: steps 1, zero `test_result` (red case `:849`) | `Run::open` through the library API with `GateRun` before another validator on a protocol whose admissible actions are all validator-owned (the shape of the unit's own fixture). Not the CLI today: it registers `[tests, gate]` and `story-delivery/1` always offers `repository.inspect`/`tests.write`/`implementation.edit`/`ci.read`, so `offered` is never empty there. Fix named, not applied: set `last_idle` only when no other validator is due at this decision, or move the `:250` check after `due` so a due validator always runs first | CONFIRMED | introduced | warning |
| 2 | `crates/takt/src/run.rs:290` (record) before `:303` (artifact) | `add_evidence` lands `gate-result-3` pass at HEAD1, then the bundle write fails and `run()` returns `Err`; the case holds a `pass` whose summary says "retained as gate-result-3.bundle.json" and nothing is retained; `gate.green` true (red case `:936`) | constructed by a harness that locks the case's `artifacts/`; the real reach is any failed write (full disk, quota), not shown. Fix: write the artifact before the record | INFEASIBLE | introduced | note |
| 3 | `crates/takt-cli/src/main.rs:248-249` | a relative `--gates` through a symlink pointing outside is accepted (no `..`), `is_file()` follows the link, the case is opened, and the library aborts step 1 with "input symlink escapes source root" (red case `gate_cli.rs:274`); the absolute form of the same path is refused at parse | any symlinked gates file or `.engineering/` directory, including the default path. Fix: canonicalise `repo.join(&path)` for relative paths too and `strip_prefix(repo)` | CONFIRMED | introduced | note |
| 4 | `crates/takt-cli/src/main.rs:185` (with `:188-190` and `validator.rs:191-195`) | `--project gates/nope.yaml` with the default project file present: the flag is dropped (`is_file()` false), `GateRun.project` is `None`, the validator falls back to `.engineering/assertions-project.yaml`, the command gate passes, exit 0 (red case `gate_cli.rs:304`) | any mistyped `--project`; the project file is "trusted admission data" (`lib.rs:33`), so the commands run were admitted by a file the user did not name. Fix: `Option<PathBuf>` for the flag with the default applied in code and a refusal when a path given explicitly is not a file; `GateRun` should take the resolved path and not fall back on its own | CONFIRMED | introduced | warning |
| 5 | `crates/takt-cli/src/main.rs:186` | an explicit `--gates` to a missing file is silently not registered and `gate.run` becomes a judgement step (pass-1's directory case pins the behaviour: exit 1 "no harness: a judgement step among gate.run is left"); with the claude harness the model is asked to run the gate and can produce no evidence; M6 survives | any mistyped `--gates`. Same fix as 4 | CONFIRMED | introduced | note |
| 6 | `crates/takt/tests/gate_validator.rs:145` | the unit's retained-bundle case checks only `gate-result-1.bundle.json`; M5 (constant name, every observation overwriting the last) survives it | my `every_observation_retains…` (:994) catches M5 | CONFIRMED | introduced | note |
| 7 | `crates/takt-cli/src/main.rs:188` | M8 (`exists()` for the project path) survives the suite: no case gives `--project` a directory | `--project .engineering` | CONFIRMED | introduced | note |
| 8 | `crates/takt/src/validator.rs:198` | `prepare(…, excluded_output: None)`: the library's snapshot excludes `.git`, `target`, `.engineering/assertions`, `.engineering/drafts` (`snapshot.rs:40-46`), not `.engineering/cases`, which the loop writes every pass (`decisions/<seq>.json`, records, the previous bundle, `intent.md`), so the retained bundle's `source_identity` is never the repository tree's alone and no two observations share one even at one revision (read, not measured by a case; extends pass-1 finding 10) | every observation. Fix: pass `Some(Path::new(CASES_DIR))` as `excluded_output` | CONFIRMED | introduced | note |

Pass-1 findings re-tested: 1 closed (`an_unknown_gate_does_not_starve…` green; it also catches M1, M3, M9); 2 closed (`a_command_bound_gate…` green; catches M4); 3 closed (`an_absolute_gates_path_inside…` green); 4 closed (the kept case catches M2 and M9); 5 closed (two-gate cases catch M10); 6 closed (directory case catches M7); 8 closed (bundles retained; both replay to their truths through the library); 7, 9, 10 not re-tested, the coordinator's no-ops, unchanged.

## 5. Attacked and not broken

- `case.revision(subject)` is `None`: the guard is false, the validator is due, and `run.rs:285` errors "the validator's subject has a revision" on that pass; no spin. Reached only by a protocol declaring `gate.run` without an `implementation` artifact (none shipped). Read, not run.
- Harness edits are always committed by the loop (`run.rs:402-412`), so the revision moves whenever a file changes and the set is keyed on the new one; pass-1's recommit case and my two-bundles case show the gate due again at HEAD1 (green).
- The set across a refused step: a refusal returns from `run()` (`:400`), the set dies with the run. The set across a judgement step that changes nothing: idle → `NoProgress` at the next pass (pass-1's cases, green).
- Two validators, CLI order `[tests, gate]`, gate unknown: tests first, then gate, then the harness (read against `story-delivery/1`; `offered` never empty there).
- Artifact name collisions: records go to `evidence/<n>-<id>.json`, bundles to `artifacts/`, `seq` is per pass; two bundles coexist (green case). A bundle on an unknown observation: retained (unit's case).
- Artifact surviving `discard_all`: `git clean -fdq -e .engineering/cases` (`git.rs:67`) excludes the directory and everything under it. Read, not run.
- Invalid project YAML or a project admitting nothing: `prepare` errors (`lib.rs:210-213`, `:403-406`) and the run aborts with `Err`, no record. Read; the gates-document error path is what pass 1 ran.
- `repo` is canonical before `under_repository` (`main.rs:139-141`), so a symlinked `--repo` or `--repo .` strips correctly; an absolute `--gates` through a symlink inside the repository to a file inside resolves and registers.
- Summary naming the deciding gate, pass/fail/unknown records, `gate.green` true/false/unknown, registration only when the file is a file: pass-1 and the unit's cases green; M7 and M10 caught.
- Replay: both retained bundles deserialise as `engineering-gate-evidence/1` and `evaluate` reproduces False and True (green case).

## 6. Paths written outside the worktree

The unit's scratch `adversary-2/`: `gate_validator-additions.rs`, `gate_cli-additions.rs`, `mutate.py`, `run-mutants.sh`, `pristine/` (`run.rs`, `validator.rs`, `main.rs`), `mutant/` (1.2M source copy; its `target/`, 934M, removed by me), `alone-<case>.log` ×5, `suite-takt.log`, `suite-takt-no-fail-fast.log`, `suite-takt-cli.log`, `clippy.log`, `fmt.log`, `mutant-build.log`, `mutant-M<n>-<name>-{takt,cli}.log` ×20, `mutants-summary.txt`, `run-mutants.log`. The harness wrote its background-task output under its own session directory. Scratch trees under the worktree's `target/tmp/` were removed by their guards (listing empty after each run).

```findings
- file: crates/takt/src/run.rs
  line: 317
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "With an unknown gate and no judgement action offered the idle step is set even though another validator is admissible with no evidence, so the next pass returns NoProgress before that validator runs; reachable through the library API with the gate validator ordered before another, not through the CLI's fixed order."
- file: crates/takt/src/run.rs
  line: 290
  category: concurrency
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "The record is written before its bundle, so a failed artifact write leaves a pass gate_result at HEAD whose summary says \"retained as\" a bundle that does not exist; constructed by a harness locking the case's artifacts directory, the real reach (a failed write) not shown."
- file: crates/takt-cli/src/main.rs
  line: 248
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "A relative --gates path through a symlink pointing outside the repository is accepted and registered, a case is opened, and the library aborts the first observation with \"input symlink escapes source root\", while the absolute form of the same path is refused at parse."
- file: crates/takt-cli/src/main.rs
  line: 185
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "An explicit --project naming a file that does not exist is dropped and GateRun falls back to .engineering/assertions-project.yaml, so the gate's commands are admitted by a project file the user never named and the run passes with exit 0."
- file: crates/takt-cli/src/main.rs
  line: 186
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "An explicit --gates naming a file that does not exist is silently not registered and gate.run becomes a judgement step no harness can produce evidence for; mutant M6 (canonicalize failure as an error) survives because no case gives an absolute path that does not exist."
- file: crates/takt/tests/gate_validator.rs
  line: 145
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The unit's retained-bundle case checks only the first observation's bundle, so a constant artifact name that overwrites every earlier bundle (M5) survives it; the added two-bundles replay case catches it."
- file: crates/takt-cli/src/main.rs
  line: 188
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "is_file() replaced by exists() for the project path (M8) survives the suite; no case gives --project a directory."
- file: crates/takt/src/validator.rs
  line: 198
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "prepare is given no excluded_output, so the retained bundle's source_identity hashes .engineering/cases, which the loop writes every pass, and no two observations share an identity even at one revision; passing CASES_DIR as excluded_output fixes it."
```
