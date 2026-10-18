---
format: aep.planning-md/3
id: review-result:adversary-validator-pass-1
kind: review-result
status: active
title: Adversary, validator-confinement, pass 1
relations:
- reviews: story:validator-confinement
revision: 1
---
I reproduced the two original high findings against this tree and both are closed. Six new cases fail: a test command can still forge a grant, leak `GH_TOKEN` and flip the gate verdict by three other routes.

```
unit: story:validator-confinement, branch impl/validator-confinement at 545379f plus uncommitted crates/takt/tests/validator_confinement_adv1.rs
verdict: CONFIRMED (blocker), 6 red cases
cases: executed 153→161, red 6
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 4 paths (part 6)
needs-coordinator: pin the base commit when the case opens (branch.yaml / case.rs, not this unit's files); remove credentials from takt's own git calls (git.rs, not this unit's)
```

**1. Diff (proof of what I touched)**
`git --no-pager diff --stat` is empty because my only change is a new untracked file. `git status --short` shows `?? crates/takt/tests/validator_confinement_adv1.rs` and nothing else. No implementation path was touched.

**2. Cases added** in `<trees>/w6-validator-confinement/crates/takt/tests/validator_confinement_adv1.rs`. The first run of the file alone was red 6, green 2, exit 101 (log `<wave-scratch>/scratch/validator-confinement/adv1/red-run.log`).

| case | asserts | now |
|---|---|---|
| `original_finding_1_a_forging_test_command_merges_nothing` | the review's forging test command (forge CI failing): nothing merged, no grant, no forged record | green: **finding 1 closed** for direct writes |
| `original_finding_2_a_gate_rewritten_on_the_case_branch_does_not_pass` | the review's gate rewrite: no `gate_result pass` | green: **finding 2 closed** for a rewrite on the case branch |
| `a_test_command_that_plants_core_fsmonitor_leaves_no_grant` | the test command only writes `core.fsmonitor`; no `authority.yaml` remains | red: `a grant the test command planted is in the case: - capability: repository.merge decision: granted ...`, and `fsmonitor = sh '.../tests/run.sh'` is still in `.git/config` |
| `a_test_command_that_plants_core_fsmonitor_gets_no_credential` | re-runs itself with `GH_TOKEN=fake`; the planted command does not see `GH_TOKEN` | red: `code the test command planted ran with GH_TOKEN` |
| `a_test_command_that_moves_the_base_branch_does_not_change_the_gate_verdict` | the test command moves `main` with `commit-tree` and `update-ref`; every gate record is still `fail` | red: `main moved from a5d70805… to 0beff06d…`; `gate-result-5` `result: pass`; the log reads `from main at 0beff06d: pass` |
| `a_test_command_that_leaves_a_detached_writer_leaves_no_grant` | a `setsid` writer sleeps 1 s, then writes the grant; none after 3 s | red: `a grant the test command's detached writer forged is in the case` |
| `run_child_leaves_no_process_that_writes_on` | the `run_child` doc (forge.rs:301): nothing the child started writes after it returns | red: `a process the child started wrote …/late after run_child returned` |
| `a_gate_command_script_rewritten_on_the_case_branch_does_not_change_the_verdict` | the project file comes from base and admits `check.sh`; the model edits `check.sh` to `exit 0`; the gate stays `fail` | red: `gate-result-5` `result: pass` |

**3. Gate run, after the cases existed** (log `<wave-scratch>/scratch/validator-confinement/adv1/gate.log`)
- `cargo fmt -p b10x-takt --check`: FMT_EXIT=0
- `cargo clippy -p b10x-takt --all-targets --locked -- -D warnings`: the first run failed (CLIPPY_EXIT=101, `unused import: Stop`, in my file). I removed the import and the rerun finished with no warnings.
- `cargo test -p b10x-takt --locked --no-fail-fast`: TEST_EXIT=101
  - test result lines in order: lib 15, forge 35, frontier 15, gate_validator 16, loop 2, merge_under_authority 30, story_delivery 13, tree_hygiene 14, validator_confinement 13: all `ok`
  - `validator_confinement_adv1`: `test result: FAILED. 2 passed; 6 failed`
  - `<before>` 153 is the same run with my file deselected.

**4. Findings** (tree 545379f)

| # | file:line | what was measured | what reaches it | verdict | origin |
|---|---|---|---|---|---|
| F1 | crates/takt/src/run.rs:688-702 | the guard compares the cases directory, then runs `git::changed_paths` (`git status`). That runs whatever the test command put in `core.fsmonitor`, and the grant lands after the comparison. tree-hygiene's `Checkpoint::put_back_git_dir` exists for exactly this, and the validator branch does not use it. | the test command is model-written: `implementation.edit` writes `tests/run.sh`, as in the implementor's own forging test | CONFIRMED, blocker | introduced |
| F2 | crates/takt/src/git.rs:21 (via run.rs:702) | the planted command runs inside takt's own git with takt's full environment, `GH_TOKEN` included. This voids the acceptance item "the test command … without GH_TOKEN". | same as F1 | CONFIRMED, blocker | introduced |
| F3 | crates/takt/src/validator.rs:191 | `base_revision` resolves the base branch by name at every observation. A test command moves `refs/heads/main` without touching the tree and decides its own gate. | test command; no guard checks refs other than `HEAD` | CONFIRMED, blocker | introduced |
| F4 | crates/takt/src/forge.rs:284-303 | `kill_group` only reaches the child's process group. A `setsid` descendant outlives it and writes after the guard has compared. The doc says it cannot. | test command; `setsid` is on every Linux | CONFIRMED, blocker | introduced |
| F5 | crates/takt/src/validator.rs:259 (`GateRun::observe`) | the gates and project file come from base, but the command script they admit is read from the case branch. The stated decision lets the model turn a command-bound gate green. | `implementation.edit` on a tracked script | CONFIRMED, warning | introduced |

Fixes I would name but did not apply:
- **F1, F2:** take a `git::Checkpoint` before `observe` and run `put_back_git_dir` before any git call afterwards; refuse when it differs.
- **F3:** record the base commit in `branch.yaml` when the case opens and read it from there (case.rs, coordinator).
- **F4:** make takt a child subreaper (`PR_SET_CHILD_SUBREAPER`) and kill every descendant, or correct the doc and the guard's claim.
- **F5:** pin admitted executables to base as well, or say in the story that command-bound gates observe model-editable code.

Note: the tree_hygiene rewrite of `a_step_that_edits_a_path_already_uncommitted…` leaves no test that reaches `StepChange::already_uncommitted`. That scenario now looks infeasible, so it is residue only.

**5. Attacked and could not break**
- Harness flags: `--restricted` removes Bash and blocks writes to git and settings files in `-p` mode, per `claude --help` 2.1.302. The argument list is correct.
- The `gh` reads name `--repo`.
- The project file is read from base.
- The dirty-tree refusal happens before observe.
- Direct writes to the cases directory or the tree are refused and undone.
- Output is bounded and timeouts fire.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/validator-confinement/adv1/fsm/` (a scratch git repo for the fsmonitor check)
- `<wave-scratch>/scratch/validator-confinement/adv1/fsm-marker`
- `<wave-scratch>/scratch/validator-confinement/adv1/red-run.log`
- `<wave-scratch>/scratch/validator-confinement/adv1/gate.log`

The test scratch under `<worktree>/target/tmp/adv1-*` is removed on drop; none is left. No process of mine is left running. The four `adv1` processes still listed belong to the tree-hygiene adversary.

```findings
- file: crates/takt/src/run.rs
  line: 702
  category: acceptance
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: the validator guard runs git status after comparing the cases directory, so a core.fsmonitor the test command planted writes a grant that survives the run
- file: crates/takt/src/git.rs
  line: 21
  category: acceptance
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: code the test command plants in the git config runs inside takt's own git calls with takt's full environment, GH_TOKEN included
- file: crates/takt/src/validator.rs
  line: 191
  category: acceptance
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: the base revision is resolved from the base branch name at every observation, so a test command that moves refs/heads/main decides the gate verdict
- file: crates/takt/src/forge.rs
  line: 301
  category: contract-drift
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: run_child kills only the child's process group, so a setsid descendant writes a forged grant after the guard compared, contrary to its doc
- file: crates/takt/src/validator.rs
  line: 259
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the command script a base-revision project file admits is read from the case branch, so editing it turns a command-bound gate green
```
