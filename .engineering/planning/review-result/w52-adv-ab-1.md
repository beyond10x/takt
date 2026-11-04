---
format: aep.planning-md/3
id: review-result:w52-adv-ab-1
kind: review-result
status: active
title: Wave 52 units a and b, adversary pass
summary: four findings, two introduced; all fixed
relations:
- reviews: story:a-relative-path-entry-is-no-test-program-directory
- reviews: story:two-trials-started-in-one-second-have-no-case-order
revision: 1
---
# Adversary, wave 52 units a and b

```
unit: story:a-relative-path-entry-is-no-test-program-directory + story:two-trials-started-in-one-second-have-no-case-order
verdict: red on two built states, neither reachable by a run
cases: executed 52→59, red 2
origin: introduced 2, pre-existing 2, undecided 0
wrote-outside-worktree: the adversary's run logs in the wave's scratch directory
needs-coordinator: no
```

Tree `takt-w52-int` at `wave/52` ec855c034, plus two untracked test files, `crates/takt/tests/w52_adversary.rs` and
`crates/takt-cli/tests/w52_adversary.rs`.

## Cases

| case | asserts | at ec855c034 | after the fix |
|---|---|---|---|
| `relative_and_empty_entries_beside_an_absolute_one_are_all_dropped` | `::`, a leading or trailing `:`, `.`, `./bin`, `..` and a bare name beside `/usr/bin` leave `/usr/bin` alone | green | green |
| `a_path_of_only_relative_entries_leaves_no_entry_that_reads_as_the_working_directory` | a `PATH` of only relative entries leaves no empty `PATH`, or the binary refuses to run | **red** | green |
| `three_trials_in_a_row_are_in_start_order` | three `takt scenario run` trials in a row take ascending case ids | green | green |
| `a_trial_after_one_at_the_largest_nonce_of_its_second_sorts_after_it` | a trial after one at nonce 99999999 of its second sorts after it | **red** | not kept: the wrap is documented |
| `after_the_wrap_a_taken_case_branch_and_case_directory_are_passed_over` | after the wrap the free search passes over a taken case branch and case directory | green | green |
| `a_remotes_trial_counts_and_other_names_do_not` | a remote's trial of the second counts; another scenario's case, a non-digit and a 23-digit suffix do not | green | green |

Also run: the two cases unit a fixed and `tests_have_a_home_of_their_own` under eight `PATH` spellings, 8 of 8 green;
`decline_adv1::passing_over_a_declined_case_does_not_pass_over_its_storys_other_open_case` 30 of 30 green (a trial
takes about 2 s on this host, so that loop rarely had two trials in one second; the same-second path is the unit test's).

## Findings

| # | where | verdict | origin | finding | outcome |
|---|---|---|---|---|---|
| F1 | `crates/takt/tests/support/own_home.rs:226` | infeasible | pre-existing | a `PATH` of only relative entries became `PATH=""`, one empty entry, which `execvp` and the step guard read as the working directory | fixed: such a `PATH` is refused before `main`, with an exit status, not an abort, so no core file is written |
| F2 | `crates/takt-cli/src/main.rs:3880` | infeasible | introduced | once a second's nonces pass 99999999 the next trial wraps to 00000000 and sorts first, against the unqualified start-order promise | fixed in the text: the doc of `unique_trial` and `takt.work.TrialNumber` name the wrap; a first trial reaches it with a chance of about k in 10^8 |
| F3 | `crates/takt-cli/src/main.rs:5412` | confirmed | introduced | after the changed assertion no unit test reaches a taken case, case branch or remote trial in the free search | fixed: the adversary's two green cases on the wrap and on remote trials are kept |
| F4 | `crates/takt/tests/tests_have_a_home_of_their_own.rs:116` | confirmed | pre-existing | `no_relative_entry_is_left_on_the_path` reads only the inherited `PATH`, so it cannot fail where that has no relative entry | fixed: the adversary's case builds its own `PATH`s |

The changed assertion in `a_trial_number_without_origin_is_the_instant_and_a_free_nonce` encoded the old start point; its
coverage of a free random nonce moved to the added second-31 assertion, and F3's lost coverage is restored by the kept
cases. The change does not weaken the test.
