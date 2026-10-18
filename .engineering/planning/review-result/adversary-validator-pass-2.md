---
format: aep.planning-md/3
id: review-result:adversary-validator-pass-2
kind: review-result
status: active
title: Adversary, validator-confinement, pass 2
relations:
- reviews: story:validator-confinement
revision: 1
---
Everything checks out. Here is my report.

```
unit: story:validator-confinement, worktree <trees>/w6-validator-confinement at 90cabe1 plus uncommitted crates/takt/tests/validator_confinement_adv2.rs
verdict: CONFIRMED (warning)
cases: executed 165→167, red 1
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 paths (part 6)
needs-coordinator: no
```

**Correction 1 holds for pass 1.** All 8 of `validator_confinement_adv1.rs` now pass against this tree: F1 (core.fsmonitor), F2 (GH_TOKEN in takt's git), F3 (moving `refs/heads/main` on a pinned case), F4 (setsid descendant), F5 (admitted script rewritten on the case branch) are closed. I did not re-raise them.

**1. `git --no-pager diff --stat`** — empty. The only change is one untracked test file:
`?? crates/takt/tests/validator_confinement_adv2.rs`. No implementation path touched.

**2. Cases added** in `<trees>/w6-validator-confinement/crates/takt/tests/validator_confinement_adv2.rs` (first run of the file alone: 1 red, 1 green, log `<wave-scratch>/scratch/validator-confinement/adv2/red.log`):

| case | asserts | now |
|---|---|---|
| `a_legacy_case_without_a_base_pin_lets_a_reset_of_main_decide_the_gate` | on a case whose `branch.yaml` has no `base_commit` (one opened before correction 1), a test command that resets `refs/heads/main` to HEAD must not change the gate verdict: every `gate_result` stays `fail` | red |
| `a_pinned_case_defeats_a_reset_of_main` | the same attack on a freshly opened (pinned) case leaves the gate `fail` | green (control — the pin works) |

Red output captured when the case was written, verbatim (the run of that case alone, before the suite):
```
thread 'a_legacy_case_without_a_base_pin_lets_a_reset_of_main_decide_the_gate' (596621) panicked at crates/takt/tests/validator_confinement_adv2.rs:202:5:
main moved from 464e543b... to 2d538538...; a gate rewritten on the case branch passed on a legacy case: [ ... "id":"gate-result-2" ... "result":"fail" ... , ... "id":"gate-result-5" ... "result":"pass","subject_revision":"2d538538..." ]
```
The run trace shows the mechanism: honest gate at step 2 is `fail` (`gate check false: fs.file.exists("LICENSE")`); after `implementation.edit` commits the rewritten gates and `tests.run` resets main to HEAD, step 5 reads `the gates document .engineering/gates.yaml from the merge-base of HEAD and main, 5d2bbd4e (the case pins no base commit): pass`.

**3. Suite run, after the cases existed** (log `<wave-scratch>/scratch/validator-confinement/adv2/suite.log`), `cargo test -p b10x-takt --locked --no-fail-fast`, exit 101:
lib 15, forge 35, frontier 15, gate_validator 16, loop 2, merge_under_authority 30, story_delivery 13, tree_hygiene 14, validator_confinement 17, validator_confinement_adv1 8: all `ok`; `validator_confinement_adv2: FAILED. 1 passed; 1 failed`. `<before>` 165 is this run with my file's two cases removed (165 + 2 = 167). `cargo clippy -p b10x-takt --tests --locked -- -D warnings` exits 0, so the red is behavioral, not a typo.

**4. Finding** (worktree at 90cabe1)

| # | file:line | what was measured | what reaches it | verdict | origin |
|---|---|---|---|---|---|
| A | crates/takt/src/validator.rs:218 | for a case whose `branch.yaml` has no `base_commit`, `base_revision` falls back to `merge-base HEAD main^{commit}`. A test command runs `git update-ref refs/heads/main HEAD`, so the merge-base collapses onto HEAD and the gate reads its gates document (and project file) from the model-edited branch. `adv2.rs:202` asserts every `gate_result` is `fail`; `gate-result-5` is `pass`. | a case opened before correction 1 recorded the pin — the migration state the code's own comment names ("Absent for a case opened before it was recorded"); the test command is model-written (`implementation.edit` writes `tests/run.sh`). New cases are pinned and immune (control case green). | CONFIRMED, warning | introduced |

This is the F3 defect revived on the one path correction 1 left on branch-name resolution. It is scoped to legacy, unpinned cases; the pin closes it for everything opened after the correction ships, so it is migration-bounded, not a blocker. The acceptance ("a gate rewritten on the case branch does not change the verdict") is nonetheless violated for those cases. Fix I would name but did not apply: for an unpinned case, record the pin on first observation (or refuse), rather than trusting `merge-base` against a branch a test command can move onto HEAD.

**5. Attacked and could not break**
- core.fsmonitor / hooks / credential helper / sshCommand planted in **local** `.git/config`: `put_back_git_dir` (run.rs:690 in the validator branch, run.rs:823 in the judgement branch) restores the guarded `.git` before the next git call, and `NO_CONFIGURED_CODE` neutralises the keys regardless. adv1's fsmonitor case now passes.
- GH_TOKEN inside takt's own git: `git::output` calls `without_credentials` and removes `GIT_CONFIG_PARAMETERS/COUNT`; even a global-config filter would run without credentials.
- Moving `refs/heads/main` on a pinned case: the pin is a raw SHA, unaffected by ref moves (control case green).
- Moving `main` **forward** on an unpinned case (adv1's `commit-tree` child): the merge-base stays at the fork point; only a reset onto HEAD defeats it.
- setsid descendant (F4): the Linux subreaper sweep reaps it before it writes; adv1's detached/`run_child` cases pass. The `RUNNING` set keeps a sweep off a concurrent call's child and its in-group descendants.
- F5 symlinked / indirectly-called executables: require the symlink to exist at the base commit (the repo author's choice, not the model's) — not reachable by a model edit, which changes the admitted path's object and is caught.
- `base_commit` pin rewrite: `branch.yaml` is under the cases directory, guarded by the snapshot/restore; a direct write is reverted.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/validator-confinement/adv2/red.log`
- `<wave-scratch>/scratch/validator-confinement/adv2/suite.log`

Scratch test trees live under `<worktree>/target/tmp/adv2-*` (CARGO_TARGET_TMPDIR, removed on drop); none left mine to clean. No process of mine is running.

Acknowledged residue I did not count (brief: only countable if cheaper than stated, and these are not): a **global** `~/.gitconfig` filter driver runs inside takt's git during `status` (NO_CONFIGURED_CODE's comment states filter drivers are uncovered, and global config is unreset) — but it runs without credentials and this is exactly the named residue, no cheaper; a process that rejoins takt's own group and drops the mark survives the sweep; an admitted script that calls a non-admitted script or reads a non-admitted data file.

```findings
- file: crates/takt/src/validator.rs
  line: 218
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: for a case with no base_commit pin (one opened before correction 1), base_revision falls back to merge-base of HEAD and the base branch, so a test command that resets refs/heads/main to HEAD collapses the merge-base onto HEAD and the gate is read from the model-edited branch, reviving finding 3 for every in-flight case across the pin migration
```
