---
format: aep.planning-md/3
id: review-result:w51-adv-b-1
kind: review-result
status: active
title: Wave 51 unit b and unit a's correction, adversary pass
summary: 'three introduced findings: codex prompt fit with context, resumed case''s tree, context help'
relations:
- reviews: story:harness-flags-spell-metaharness-names
- reviews: story:harness-section-is-metaharness-session-request
revision: 1
---
# Adversary, wave 51 unit b and unit a's correction (unit a's second pass)

```
unit: story:harness-flags-spell-metaharness-names (+ unit a correction)
verdict: red
cases: executed 0→6, red 3
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: the adversary's run log in the wave's scratch directory
needs-coordinator: no
```

Tree `takt-w51-b` at `wave/51` 8d18d75b2, plus one untracked test file, `crates/takt-cli/tests/w51_b_adversary.rs`.

## Cases

| case | asserts | now |
|---|---|---|
| `codex_context_beside_a_scope_file_is_started_or_refused_before_any_case_opens` | Codex with `--context ctx.md` (65,528 bytes against the cap) and an intent whose `## Scope` names `big.md` (65,400 bytes): either the codex double starts, or nothing opens. A control run with the scope file alone starts the double. | **red** |
| `a_resumed_cases_context_is_checked_against_the_cases_branch` | The case `probe` is opened, then `later.md` is committed on `main`. `takt run --case probe --context later.md` from `main` either hands over the file or is refused by takt itself. | **red** |
| `scenario_run_help_describes_context_as_the_run_checks_it` | A scenario run refuses a file HEAD tracks and the seed does not (green part); `takt scenario run --help` must not say "a file the repository's index tracks". | **red** |
| `a_context_list_at_the_cap_reaches_the_session_and_one_byte_past_it_is_refused` | 65,536 bytes reach claude; one byte more is refused before any case opens, naming `c.md`. | green |
| `scenario_run_hands_a_context_file_the_seed_holds_and_head_does_not` | A file only the seed holds reaches the session. | green |
| `scenario_run_context_flag_replaces_the_files_list` | The file's `context: [absent.md]` plus `--context notes.md` gives the note, with no refusal. | green |

```
thread 'codex_context_beside_a_scope_file_is_started_or_refused_before_any_case_opens' panicked at crates/takt-cli/tests/w51_b_adversary.rs:322:5:
a context list takt admitted opened a case whose codex session was never started: ...
stopped: error: step 2: the harness failed; ...: the session proposed nothing: metaharness refused the session: the adapter refused to plan the launch: the prompt would be 134790 bytes, and Linux refuses one argument of 131072 bytes or more, its terminating NUL included (MAX_ARG_STRLEN), so the spawn would fail with E2BIG; the --context files are ctx.md (65528 bytes). ...

thread 'a_resumed_cases_context_is_checked_against_the_cases_branch' panicked at crates/takt-cli/tests/w51_b_adversary.rs:363:5:
the resumed case's session could not be handed later.md, and takt did not refuse it before the case resumed: ...
stopped: error: step 1: the harness failed; ...: the session proposed nothing: the context file "later.md" is not one of the files the session's copy holds, spelled as one plain relative path; ...

thread 'scenario_run_help_describes_context_as_the_run_checks_it' panicked at crates/takt-cli/tests/w51_b_adversary.rs:476:5:
`takt scenario run --help` promises --context takes a file the repository's index tracks, which it refuses: ...
```

Suite: `cargo test -p b10x-takt-cli --test w51_b_adversary`: `test result: FAILED. 3 passed; 3 failed; 0 ignored; 0
measured; 0 filtered out; finished in 2.18s`, exit 101.

## Findings

1. `crates/takt/src/harness.rs:951` (the Codex `fit` closure), NEEDS-CHANGE, warning, introduced by unit a. takt fits
   the scope and instruction files into Codex's one prompt argument without counting the `context` files metaharness
   appends to it; both budgets plus the prompt reach 134,790 bytes, past 131,072. The case opens and the session is
   refused at step 2, while `check_context` (`run_config.rs:659`) says it checks as metaharness would at launch.
   Reached by Codex with `context` near the cap and a story whose `## Scope` files fill the scope budget. Fix: fit with
   the requested context files counted, or shrink the scope budget by their cost.
2. `crates/takt-cli/src/main.rs:861` (and `:845`), NEEDS-CHANGE, warning, introduced by unit a. `takt run` checks
   `context` against the work tree when the command is read; a resume checks out the case's branch afterwards, so a
   file `main` tracks and the case's branch does not is admitted and refused by Loom at the step (the reverse,
   inferred, not tested). Reached by `takt run --case <id> --context …` and the default resume. Fix: check against the
   case's tree once it is known.
3. `crates/takt-cli/src/main.rs:1204`, NEEDS-CHANGE, note, introduced by unit b. The shared `LoopArgs` help says
   `--context` is "a file the repository's index tracks", which `takt scenario run` refuses when its seed lacks it;
   it omits the size, text and credential rules.

## Attacked and could not break

The exact byte cap; a seed-only file in a scenario run; the flag over the file's context; a file-wins mutant (caught
by `a_flag_wins_over_the_files_max_turns`); the `--strict-version` observable (a mutant dropping takt's preflight
turns it red; one keeping the preflight and dropping the setting cannot be seen with doubles, as both use the same
pin); `decimal_of` on exponent forms (above 15 significant digits rounds, not raised); `ls-tree` modes, the NUL and
UTF-8 checks, a credential named by its shape only.

```findings
[
  {"file": "crates/takt/src/harness.rs", "line": 951, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "under Codex the scope-file fit ignores the requested context files metaharness appends to the same prompt argument, so a context list within the cap beside a full scope budget opens a case whose session is refused at launch for E2BIG (codex_context_beside_a_scope_file_is_started_or_refused_before_any_case_opens)"},
  {"file": "crates/takt-cli/src/main.rs", "line": 861, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "takt run checks context against the work tree before a resumed case checks out its own branch, so a file the case's branch lacks is admitted and refused by Loom at the step (a_resumed_cases_context_is_checked_against_the_cases_branch)"},
  {"file": "crates/takt-cli/src/main.rs", "line": 1204, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the shared --context help says a file the repository's index tracks, which takt scenario run refuses when its seed lacks the file (scenario_run_help_describes_context_as_the_run_checks_it)"}
]
```
