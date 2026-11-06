---
format: aep.planning-md/3
id: review-result:w54-adv-1
kind: review-result
status: active
title: Wave 54 units a, c, b and d, adversary pass
summary: 'six red cases: five fixed by the correction, one escalated as a draft'
relations:
- reviews: story:a-git-read-error-leaves-no-process
- reviews: story:resume-names-a-missing-branch
- reviews: story:test-cmd-checked-before-open
- reviews: story:gate-flag-names-a-protocol-action
revision: 1
---
# Adversary, wave 54 units a, c, b and d

```
unit: story:a-git-read-error-leaves-no-process + story:resume-names-a-missing-branch + story:test-cmd-checked-before-open + story:gate-flag-names-a-protocol-action
verdict: needs change
cases: executed 13 new, red 6
origin: introduced 5, undecided 1
```

The cases are `crates/takt/tests/w54_adversary.rs` and `crates/takt-cli/tests/w54_adversary_cli.rs`.

| finding | unit | case | outcome |
|---|---|---|---|
| B2: on a resume the test program was checked in the tree before the case branch was checked out | b | `w54_b_a_resume_from_another_branch_checks_the_script_on_the_case_branch` | fixed |
| C1: a case branch kept only on its one remote was refused, though git's checkout recreated it | c | `w54_c_a_branch_kept_on_origin_still_resumes` | fixed |
| C2: the printed `git branch` line and the directory were not shell-quoted | c | `w54_c_the_printed_git_branch_line_is_safe_to_paste` | fixed |
| B1: empty, `.` and relative `PATH` entries, which `execvp` searches from the tree, were skipped | b | `w54_b_check_program_agrees_with_the_step_on_an_empty_path_entry` | fixed |
| B4: any execute bit passed the check, though the step meets `access(X_OK)` | b | `w54_b_check_program_agrees_with_the_step_on_an_execute_bit_for_others_only` | fixed |
| D1: `--case-id` dropped when a story run resumes an open case, likely older than the wave | d | removed from the file, kept with its draft | escalated: story:a-case-id-on-a-story-resume-is-refused |

Attacked and held: a bounded git read past its limit and on a failing git leaves no process; a tag named like the
branch is no branch; a refused resume writes no `.gitignore` and no hold; an empty `--gate` and an action needing no
capability are refused with nothing written; dangling links, directories and links out of the tree agree with the
step.

Decided by approval-record:wave-54-adversary-correction; the correction is 7501e6380. CI on the pull request then found
three library tests the wave had made red (a test process started outside the confinement seam, a quoted test command
an older case built, and `/proc` reads on macOS), fixed in e6012f1e9.
