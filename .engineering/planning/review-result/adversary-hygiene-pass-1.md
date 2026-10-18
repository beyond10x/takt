---
format: aep.planning-md/3
id: review-result:adversary-hygiene-pass-1
kind: review-result
status: active
title: Adversary, tree-hygiene, pass 1
relations:
- reviews: story:tree-hygiene
revision: 1
---
unit: story:tree-hygiene, commit 9618c80 on ae94ccb, plus my one untracked test file
verdict: NEEDS-CHANGE
cases: executed 140→146, red 6
origin: introduced 7 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory (`<wave-scratch>/scratch/tree-hygiene/adv1/logs/`, 5 log files)
needs-coordinator: yes. F3 is a design choice: putting the shared git directory back destroys other processes' writes, and not putting it back lets planted code run.

**1. Diff stat**
`git --no-pager diff --stat` prints nothing because my only change is an untracked file. `git status --short` gives `?? crates/takt/tests/tree_hygiene_adv1.rs`. That is a test path only. No implementation file was touched and nothing was committed.

**2. Cases added** (`<trees>/w6-tree-hygiene/crates/takt/tests/tree_hygiene_adv1.rs`, all 6 red now, each run alone before the suite)

| case | asserts | red output when run alone (verbatim) |
|---|---|---|
| `an_editors_config_write_in_the_shared_git_directory_during_a_step_survives_it` | another worktree's `git config branch.main.vscode-merge-base` made during a step survives the step | `the other worktree's config write was reverted by the step's guard (StepRefused("implementation.edit wrote under .git: …/main/.git/config"))` |
| `a_planning_store_lock_made_in_another_worktree_during_a_step_survives_it` | a `.git/aep/planning-writer-*.lock` made during a step survives the step | `another process's lock file was deleted by the step's guard (StepRefused("implementation.edit wrote under .git: …/.git/aep/planning-writer-store-0123abcd.lock"))` |
| `a_step_that_rewrites_an_ignored_file_that_existed_is_refused_and_the_file_put_back` | rewriting an existing `override.local` (PASS=0 to PASS=1) is refused and the file put back | `panicked at …:217:5: Iterations(2)` with log line `effect: committed 1 path(s)` |
| `a_step_that_leaves_an_unreadable_file_under_git_is_refused_and_nothing_it_planted_stays` | a hook plus a mode-000 file under `.git/info/` gives StepRefused and leaves no hook | `a write under .git ended the run with an error instead of a refusal; the planted hook is still there, src/lib.rs is "model\n": read …/.git/info/zz-unreadable: Permission denied (os error 13)` |
| `a_step_that_leaves_a_fifo_under_git_does_not_hang_the_run` | a FIFO under `.git/info/` does not hang the run | `the run hung for 30s reading the FIFO a step left under .git/; it went on only after 2 writer(s) opened it` |
| `a_hook_a_refused_step_planted_in_the_trees_hook_directory_does_not_run_while_it_is_undone` | with `core.hooksPath=githooks`, a refused step's `githooks/reference-transaction` does not run | `the refused step's hook ran inside takt's own git calls: preparing\nprepared\ncommitted…` |

The first draft of the FIFO case hung the test binary for 10 minutes: it unblocked the FIFO once, but the run reads it twice. I unblocked it by hand once, killed nothing, and changed the case to keep unblocking in a loop. The output in the table is from the corrected case run alone.

**3. Suite** (`cargo test -p b10x-takt --locked --no-fail-fast`, run after the cases existed)
- Every other test binary passed: lib 15, forge 35, frontier 15, gate_validator 16, loop 2, merge_under_authority 30, story_delivery 13, tree_hygiene 14, doctests 0.
- `tree_hygiene_adv1`: `test result: FAILED. 0 passed; 6 failed; … finished in 30.23s`, `EXIT=101`.
- Clippy `-D warnings` exited 0; `cargo fmt -p b10x-takt --check` exited 0.
- The `<before>` number is 146 minus my 6 cases, taken from this same run.

**4. Findings**

| # | file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|---|
| F1 | run.rs:750, git.rs:317 | One unreadable file under `.git/` makes `put_back_git_dir` stop with `?` before it puts anything back, and `undo_step` never runs. Planted hooks and config stay and the worktree change stays. This breaks the `.git/` guard. | NEEDS-CHANGE / introduced | Any step can `chmod 000` a file. I ran as uid 1000. |
| F2 | git.rs:317, 329 | `Content::of` reads a FIFO's bytes, once in `guarded()` and again in `put_back`, so the run blocks with no timeout. | NEEDS-CHANGE / introduced | Any step can run `mkfifo .git/x`. |
| F3 | git.rs:453-477, 581-599 | Writes other processes make to the shared git directory during a step are put back (deleted), and the step is refused. | NEEDS-CHANGE / introduced | This repository's own `.git` right now has `branch.impl/*.vscode-merge-base` config entries (for branches made today), `gk/config` changed at 12:14, and 18 `aep/planning-writer-*.lock` files made 03:41–11:58. The `.lock` skip applies only at the top level of `.git/`. |
| F4 | git.rs:218-225, 633, 643 | Ignored files are compared by path only, so rewriting an ignored file that already existed is neither noticed nor put back. The stated decision covers only directories that are ignored as a whole; this is a single file git lists by name. | NEEDS-CHANGE / introduced | `override.local` read by the test command, which is finding 9's own example. |
| F5 | git.rs:675 (also 661, 665) | `restore` runs `reset --hard`, `symbolic-ref` and `update-ref` while the step's files are still in the tree. A hook directory inside the tree (`core.hooksPath`) therefore runs a hook the refused step planted. The old `checkout -- .` plus `clean` changed no ref. A takt commit running a hook the model edited is pre-existing, because `commit_all` did the same. | NEEDS-CHANGE / introduced | Repositories that keep hooks in the tree (husky, a `.githooks` folder). |
| J6 | run.rs:501 | The take commit runs the repository's hooks. A commit-message check that only allows types such as `feat`/`fix` would reject `takt: take story:x`, and `open_next` would then error at every candidate. Not run. | CONFIRMED from reading the code / introduced | Repositories with a commit-message hook. |
| J7 | run.rs ~474 | A case that records story S but has no take commit is neither resumed nor reopened: it is skipped with "opened for it already", and `open_next` returns `None`. The doc comment says `None` only when nothing is open. | INFEASIBLE / introduced | Only cases made before this change whose take was reverted (defect 10's state). |

Fixes, named and not applied:
- **F1, F2:** give FIFOs, sockets and unreadable entries their own kind. Compare them without opening them, remove them, and on any error still undo the step.
- **F3:** this is the design choice for you. Either guard only what git runs or reads as config (hooks, config, info) and leave other tools' files alone, or neutralise takt's own git calls (`-c core.hooksPath=/dev/null -c core.fsmonitor=false`) instead of putting the directory back.
- **F4:** record and compare the content of ignored *files*.
- **F5:** clean the tree before any command that updates a ref, or turn hooks off for takt's git calls.

**5. Attacked and not broken**
- `status.branch` and `status.showStash` config does not leak into `--porcelain=v1 -z` output (checked on git 2.55).
- A literal-pathspec commit of a leading-space and non-ASCII path: read the code, matches the existing test.
- A step that deletes the case branch or detaches `HEAD`: `restore` makes the ref again (read, not run).
- `reset --hard` does run the reference-transaction hook (checked on git 2.55), which is what F5 rests on.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/tree-hygiene/adv1/logs/` holds `adv1-alone.log`, `adv1-fifo-alone.log`, `suite.log`, `clippy.log` and `fmt.log`.
- `<wave-scratch>/scratch/tree-hygiene/adv1/g1` was a scratch repository; I removed it.
- The test scratch directories under `<worktree>/target/tmp/hygiene-adv1-*` were removed when each test finished. None are left, and no stray processes remain.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 750, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "an unreadable file under .git makes put_back_git_dir error out before anything is put back, so planted hooks and the step's change survive and the run errors instead of refusing"},
  {"file": "crates/takt/src/git.rs", "line": 317, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Content::of reads a FIFO's bytes, so a FIFO a step leaves under .git blocks the run with no timeout, read twice"},
  {"file": "crates/takt/src/git.rs", "line": 581, "category": "concurrency", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "other processes' writes to the shared git directory during a step (editor config, aep lock files) are reverted and the step is refused"},
  {"file": "crates/takt/src/git.rs", "line": 218, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "ignored files are compared by path only, so a step that rewrites an ignored file that already existed is accepted and the file keeps the step's content"},
  {"file": "crates/takt/src/git.rs", "line": 675, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "restore runs reset --hard while the refused step's files are still in the tree, so a hook it planted in an in-tree core.hooksPath runs inside takt's own git calls"},
  {"file": "crates/takt/src/run.rs", "line": 501, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the take commit runs repository hooks, so a commit-message hook rejecting \"takt: take <story>\" makes open_next error at every candidate"},
  {"file": "crates/takt/src/run.rs", "line": 474, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a case for a story with no take commit (made before this change) is neither resumed nor reopened, and open_next returns None although a case is open"}
]
```
