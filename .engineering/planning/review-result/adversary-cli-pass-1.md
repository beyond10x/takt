---
format: aep.planning-md/3
id: review-result:adversary-cli-pass-1
kind: review-result
status: active
title: Adversary, cli-protocol-defaults, pass 1
relations:
- reviews: story:cli-protocol-defaults
revision: 1
---
unit: story:cli-protocol-defaults, impl commit 15cbfeb on base ae94ccb, worktree <trees>/w6-cli-protocol-defaults (plus one untracked test file I added)
verdict: NEEDS-CHANGE
cases: executed 173→176, red 3
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 paths (part 6)
needs-coordinator: none

**1. Diff stat**

`git --no-pager diff --stat` prints nothing, because my only change is a new untracked file. `git status --short`:
```
?? crates/takt-cli/tests/protocol_defaults_adv1.rs
```
That is a test path. I changed no implementation file and committed nothing.

**2. Cases added** (`crates/takt-cli/tests/protocol_defaults_adv1.rs`). All three are red now. Each first runs a case and leaves it, then reruns it with `--protocol story.delivery/1` on a case that runs `software.change/1`. Each asserts exit 1 and that stderr says "nothing was changed"; both pass. It then asserts the tree is unchanged, and that fails.

| case | setup before the refused run | red because |
|---|---|---|
| `adv_a_refused_protocol_for_a_case_leaves_the_tree_on_the_branch_it_was_on` | `--case change`, tree checked out back on `main` | HEAD moved from `refs/heads/main` to `refs/heads/takt/change` |
| `adv_a_refused_protocol_for_the_store_resumed_case_leaves_the_tree_on_its_branch` | the store story's case on `software.change/1` (the trial run 1 case), tree back on `main`, a plain run with `--protocol story.delivery/1` | HEAD moved to `refs/heads/takt/story-ready-one-…` |
| `adv_a_refused_protocol_for_a_case_does_not_rewrite_its_revision` | a commit on the case branch between runs | `.engineering/cases/change/case.yaml` was rewritten |

Red output of these cases run alone (`cargo test -p b10x-takt-cli --locked --test protocol_defaults_adv1`, EXIT=101), verbatim:
```
panicked at crates/takt-cli/tests/protocol_defaults_adv1.rs:214:5:
the refusal says nothing was changed, and it rewrote [".engineering/cases/change/case.yaml"]
panicked at crates/takt-cli/tests/protocol_defaults_adv1.rs:152:5:
assertion `left == right` failed: the refusal says nothing was changed, and the tree's HEAD/branch/status moved
  left: ("65a572b…", "refs/heads/takt/change", "?? .engineering/cases/change/branch.yaml …")
 right: ("65a572b…", "refs/heads/main", "?? .engineering/cases/change/branch.yaml …")
panicked at crates/takt-cli/tests/protocol_defaults_adv1.rs:180:5:
  left: ("11582fb…", "refs/heads/takt/story-ready-one-20261018T100647Z", …)
 right: ("11582fb…", "refs/heads/main", …)
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
```
Full log: `<wave-scratch>/scratch/cli-protocol-defaults/adv1/red-alone.log`.

**3. Suite run, after the cases existed:** `cargo test -p b10x-takt -p b10x-takt-cli --locked --no-fail-fast`, EXIT=101.
- The library targets passed 127 cases: 16, 35, 15, 16, 2, 30 and 13.
- The CLI targets passed 46: main.rs 17, forge_cli 3, frontier_cli 6, gate_cli 8, merge_cli 2, protocol_defaults_cli 10.
- `protocol_defaults_adv1`: `test result: FAILED. 0 passed; 3 failed`.
- Total executed: 176. The "before" figure of 173 is the same run with my target's 3 cases left out. I have no earlier `cases:` line from the implementor.
- `cargo fmt --check` exits 0. Clippy (`-p b10x-takt-cli --all-targets -D warnings`) gives no warnings.

**4. Findings**

| # | file:line | what was measured | what reaches it | verdict | origin |
|---|---|---|---|---|---|
| F1 | `crates/takt-cli/src/main.rs:411` (the check), `crates/takt/src/case.rs:626,644` (the side effects) | The mismatch check runs after `Run::resume` (main.rs:388) or `open_next`. By then `CaseDir::resume` has already switched the tree to the case branch (case.rs:626) and may have rewritten `case.yaml`'s revision (case.rs:644). The refusal then says "nothing was changed". This breaks the acceptance's "touching nothing". | The recovery from trial run 1: an operator back on `main` reruns with `--protocol story.delivery/1` to fix a case on the wrong protocol. Also any commit made on the case branch between runs. The implementor's tests miss this because their tree was already on the case branch and HEAD had not moved. | NEEDS-CHANGE | introduced (the refusal and its claim are new in this diff) |
| F2 | `crates/takt-cli/tests/frontier_cli.rs:264` | This test is now green for a different reason. `--input risk=bogus` with no `--protocol` is now refused by the new undeclared-input check (main.rs:277, "story.delivery/1 declares no input risk"). It no longer reaches `CaseDir::open`'s value check, which its doc comment names. The library still covers that check (`crates/takt/tests/frontier.rs:745`). | The default protocol changed to `story.delivery/1`, which declares no inputs | CONFIRMED | introduced |
| F3 | `crates/takt-cli/src/main.rs:277` | The input check uses the protocol a case *would* open on, but `open_next` may instead resume an active story's case on another protocol. Measured by reading the code, not by a case. `--input risk=high` with no `--protocol` on an active `software.change/1` case is refused, and the message names `story.delivery/1`. `--protocol software.change/1 --input risk=high` passes the check, and the input is then dropped without a word. | Store resume path only; rare | CONFIRMED | introduced |

The fix I name for F1, not applied: check the protocol before anything is resumed. For `--case`, read `<repo>/.engineering/cases/<id>/protocol.yaml` in main.rs, or compare in `Run::resume` (in this unit's range) before `CaseDir::resume`. For `open_next`, compare once `case::resumable` has returned the case id, before `Run::resume`.

The stated deviations:
- **Exit 5 for every `Run::run` error:** this is consistent with `EXIT_CODES`. A case held by another run actually exits 1, because `CaseDir::resume` (case.rs:618) refuses it before `hold`. Exit 5 happens only if another run takes the lock between resume and `hold`. That is not a finding.
- **Exit 4:** could not break it.
- **`--case` dropping inputs:** pre-existing, and the deviation names it.

**5. Attacked, could not break**
- The exit-code mutants: `merge_cli` pins 4 exactly, and the help-line assertions catch 4 and 5 being swapped.
- The id branch and the `ir_json` branch of `refuse_another_protocol`.
- Both embedded spellings, and `story.delivery/2` being refused.
- Every `Stop` variant is listed in `EXIT_CODES`.
- The defaults for `software.change` when the protocol is given as `software-change/1`.
- The `--forge` help text against the code.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/cli-protocol-defaults/adv1/` (directory)
- `<wave-scratch>/scratch/cli-protocol-defaults/adv1/red-alone.log`
- `<wave-scratch>/scratch/cli-protocol-defaults/adv1/suite.log`

Test scratch repositories went under the worktree's `target/` (`CARGO_TARGET_TMPDIR`) and remove themselves on drop. I took a session lease (`takt-w6-cpd-adv1`) and released it; `session-end` exited 0. No processes are left running.

```findings
- file: crates/takt-cli/src/main.rs
  line: 411
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the --protocol mismatch refusal runs after CaseDir::resume has checked the tree out onto the case branch and rewritten case.yaml's revision, so a refusal that says "nothing was changed" moves HEAD and edits the case
- file: crates/takt-cli/tests/frontier_cli.rs
  line: 264
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: with story.delivery/1 the default, the test's --input risk=bogus is refused by the new undeclared-input check, not by the CaseDir::open value check its doc comment names, so it no longer guards that path from the binary
- file: crates/takt-cli/src/main.rs
  line: 277
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the undeclared-input check uses the protocol a case would open on, while open_next may resume an active case on another protocol, so it names the wrong protocol when it refuses and lets a declared input through only for it to be dropped
```
