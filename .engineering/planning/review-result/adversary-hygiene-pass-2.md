---
format: aep.planning-md/3
id: review-result:adversary-hygiene-pass-2
kind: review-result
status: active
title: Adversary, tree-hygiene, pass 2
relations:
- reviews: story:tree-hygiene
revision: 1
---
unit: story:tree-hygiene, impl/tree-hygiene at 48688d2 (correction 1) on base ae94ccb, plus my one untracked test file
verdict: NEEDS-CHANGE
cases: executed 149→154, red 5
origin: introduced 0 / pre-existing 0 / undecided 5
wrote-outside-worktree: 1 directory (`<wave-scratch>/scratch/tree-hygiene/adv2/logs/`, 6 logs)
needs-coordinator: yes. My first run of case 1 moved the HEAD of this worktree. I put it back (details in part 2).

**1. Diff stat**
`git --no-pager diff --stat` is empty. `git status --short` shows only `?? crates/takt/tests/tree_hygiene_adv2.rs`, a test path. No implementation file was touched and nothing was committed.

**2. Cases added** in `<trees>/w6-tree-hygiene/crates/takt/tests/tree_hygiene_adv2.rs`. All 5 are red now. Each was run alone before the suite (`adv2-alone.log`).

| # | case | red output when run alone |
|---|---|---|
| A1 | `a_step_that_garbles_head_is_refused_and_its_change_discarded` | `takt's git calls after the step ran in the repository enclosing the tree and moved its HEAD … left: "ref: refs/heads/takt/garbled-head\n" right: "ref: refs/heads/main\n"`, and the run ended `Err(… git update-ref refs/heads/takt/garbled-head … nonexistent object …)` |
| A2 | `a_hook_planted_under_a_hooks_directory_made_read_only_does_not_stay` | `the hook the refused step planted is still in the repository (StepRefused("… not put back: .git/hooks/pre-commit: remove …: Permission denied (os error 13)"))` |
| A3 | `a_large_ignored_file_rewritten_with_its_size_and_time_kept_is_refused` | `a rewrite of an ignored file that kept its size, mode and time was not noticed: Iterations(2)` plus `effect: committed 1 path(s)` |
| A4 | `a_guarded_key_written_into_a_sibling_worktrees_config_is_refused_and_put_back` | `… the sibling's git now reads core.hooksPath=set-by-the-step: Iterations(2)` |
| A5 | `a_step_that_leaves_the_index_locked_leaves_no_uncommitted_change` | `the step's change was left uncommitted in the tree: " M src/lib.rs"; the run ended Err(discard the step's change … git reset -q --hard … Unable to create '…/.git/index.lock': File exists.` |

**The worktree HEAD incident.** The first draft of A1 put its scratch repository straight under `target/tmp`. Once `.git/HEAD` was garbled, git stopped treating that directory as a repository and searched upward. takt's `symbolic-ref` then landed in this worktree, so its HEAD pointed at `refs/heads/takt/garbled-head`.
- I first checked that `refs/heads/impl/tree-hygiene` was still at 48688d2. I ran nothing else beforehand.
- I then ran `git symbolic-ref HEAD refs/heads/impl/tree-hygiene`.
- Status is clean again apart from my test file, and I re-checked after each later run.

A1 now nests its scratch repository inside an outer scratch repository, so the upward search stops there.

**A second draft of A1 deleted files.** In that draft both scratch repositories had byte-identical "start" commits, so the case's commit also existed in the outer repository. takt's `update-ref` then succeeded there, and its `reset --hard` and `clean -fdq` ran in the outer worktree and deleted the inner tree (`adv2-head-alone.log`). I built that state myself. The case in the file now gives the outer repository a different commit (`adv2-head-alone-2.log`).

**3. Suite** (run after the cases existed; logs in the scratch directory)
- `cargo fmt -p b10x-takt --check`: exit 0
- `cargo clippy -p b10x-takt --all-targets --locked -- -D warnings`: exit 0
- `cargo test -p b10x-takt --locked --no-fail-fast`: exit 101
  - every other test binary passed: lib 16, forge 35, frontier 15, gate_validator 16, loop 2, merge_under_authority 30, story_delivery 13, tree_hygiene 16, tree_hygiene_adv1 6
  - `tree_hygiene_adv2`: `test result: FAILED. 0 passed; 5 failed`
- The before number, 149, is this run minus my 5 cases.

**4. Findings** (all cover 48688d2)

**Origin.** Every row is `undecided` because I have no base worktree. Each failing line was added by this unit.

| # | file:line | finding | verdict | what reaches it |
|---|---|---|---|---|
| A1 | git.rs:941-966 | `.git/HEAD` is not put back before takt's next git call. Git then runs those calls in the nearest enclosing repository: it moved that repository's HEAD (measured). When the commit also exists there, git ran `reset --hard` and `clean -fdq` in it (seen only in a state I built). Fix: pin every git call to the checkpoint's git directory (`--git-dir`/`--work-tree`, or `GIT_CEILING_DIRECTORIES`), or write `HEAD` back in `put_back_git_dir`. | NEEDS-CHANGE | Any repository nested in another: this crate's own tests under `target/tmp`, or a home-directory dotfiles repository. |
| A2 | git.rs:433-443, 826-833 | `remove()` makes a directory listable but never makes the parent writable. A step that runs `chmod 555 .git/hooks` keeps its planted hook after the refusal. `Content::Dir` does not record the directory's mode either. | NEEDS-CHANGE | Any step can run `chmod`. |
| A3 | git.rs:331-336, 354-359 | An opaque entry is compared by kind, size, mode and modification time, and a step can set all four back. The change time (`ctime`) and the inode number cannot be set back by a step and are not compared. | NEEDS-CHANGE | Ignored files over 64 MiB, such as models or databases a test command reads. Setting the time back on purpose is unusual, but `cp -p` and `rsync -t` both do it. |
| A4 | git.rs:503, 735-738 | The configuration of a sibling worktree, `.git/worktrees/<other>/config.worktree`, is not guarded. Neither is its `info/sparse-checkout`. | NEEDS-CHANGE | `extensions.worktreeConfig=true`, which `git sparse-checkout` turns on. |
| A5 | run.rs (commit failure → `undo_step(...)?`), git.rs:966 | An `index.lock` left in the tree's own git directory makes both the commit and the undo fail. The run errors and leaves the step's edit uncommitted in the tree. Only a process the step started can take that lock (it is per worktree). | NEEDS-CHANGE | A git process a step started being killed by a harness timeout. I found no observation of this happening. |

**Judgement (not made into cases):**
- **Configuration keys outside the guarded list.** Several name programs or change git's behaviour: `gpg.*` (`program`, `ssh.defaultKeyCommand`), `log.showSignature`, `sequence.editor`, `pager.*`, `trailer.*.cmd`, `difftool.*.cmd`, `mergetool.*.cmd`, `submodule.*` (`recurse`, `update`), `extensions.*`, `index.*`, `feature.*`, `user.*`. `NO_CONFIGURED_CODE` does not force `log.showSignature=false`, and `has_subject` runs `git log`.
- **Cost of comparing ignored files.** Every ignored file git lists by name is read in full, up to 64 MiB each, and held in memory once per checkpoint and again per `change()`. A build tree ignored file by file (`*.o`) costs N times the file size each step. I did not measure it.
- **Environment cleaned for git (`git::run`).** It drops 5 variables. It leaves `GIT_CEILING_DIRECTORIES`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR` and `GIT_*_PATHSPECS`. `GIT_GLOB_PATHSPECS=1` conflicts with `--literal-pathspecs`. Only the operator's environment sets these.

**5. Attacked and not broken**
- Pass 1's 6 cases now pass: FIFO, unreadable file, foreign config, foreign lock, ignored rewrite, in-tree hooks.
- `hooks/` and `info/` are walked at any depth. `info` replaced by a link is removed before its files are written back (path order; read, not run).
- A `.git/config` that is unreadable, a link, or has a syntax error is treated as differing and put back (read, not run).

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/tree-hygiene/adv2/logs/` holds 6 files: `adv2-alone.log`, `adv2-head-alone.log`, `adv2-head-alone-2.log`, `fmt.log`, `clippy.log` and `suite.log`.
- This worktree's git directory, `<repo>/.git/worktrees/w6-tree-hygiene/HEAD`: moved by the code under test, and put back by me.
- Test scratch directories under `<worktree>/target/tmp/hygiene-adv2-*` were removed when each test finished. None are left, and no processes I started are still running.

```findings
[
  {"file": "crates/takt/src/git.rs", "line": 941, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "a step that garbles .git/HEAD makes restore's git calls run in the enclosing repository, moving its HEAD, and with the commit present there reset --hard and clean run in it"},
  {"file": "crates/takt/src/git.rs", "line": 433, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "put-back never makes a parent directory writable, so a hook planted before chmod 555 .git/hooks stays after the refusal"},
  {"file": "crates/takt/src/git.rs", "line": 331, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "opaque entries are compared by kind, size, mode and mtime, all settable by the step; a rewrite of a >64 MiB ignored file with its mtime set back is committed unnoticed"},
  {"file": "crates/takt/src/git.rs", "line": 503, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "a sibling linked worktree's config.worktree under the shared git directory is not guarded, so a guarded key written there is accepted and stays"},
  {"file": "crates/takt/src/git.rs", "line": 966, "category": "concurrency", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "an index.lock left in the tree's own git directory by a step makes both commit and undo fail, so the run errors with the step's edit left uncommitted"}
]
```
