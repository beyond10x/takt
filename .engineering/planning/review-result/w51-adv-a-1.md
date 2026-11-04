---
format: aep.planning-md/3
id: review-result:w51-adv-a-1
kind: review-result
status: active
title: Wave 51 unit a, adversary pass 1
summary: 'four introduced findings: scenario seed, context launch check, micro-dollar number, refusal naming'
relations:
- reviews: story:harness-section-is-metaharness-session-request
revision: 1
---
# Adversary, wave 51 unit a, pass 1

```
unit: story:harness-section-is-metaharness-session-request
verdict: red
cases: executed 0→8, red 5
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: none
needs-coordinator: no
```

Tree `takt-w51-a`, on `unit/51-a` at 8bbfe3567. The `executed` count covers the adversary's file only.

## Cases (`crates/takt-cli/tests/w51_a_adversary.rs`)

- `a_one_micro_dollar_budget_written_as_a_number_reaches_the_session`: red. The string `"0.000001"` reaches the
  session as `--max-budget-usd 0.000001`; the YAML number `0.000001` is refused: `` `max-budget-usd` in its `harness`
  mapping is a sum of US dollars, a number or a decimal string such as 2.5, at least zero and no finer than one
  micro-dollar (six digits after the point) ``
- `a_refusal_under_codex_names_only_the_flag_codex_does_not_take`: red. `the command line's --effort, --max-budget-usd
  are refused under the codex harness, as metaharness checks the session: … the run asked for --max-budget-usd and the
  pinned codex exec has no way to express it …`
- `a_tracked_context_file_over_the_byte_cap_is_refused_before_any_case_opens`: red. A case opens, the tests run, then
  step 2: `metaharness refused the session: --context docs/long.md is refused: with it the context files reach 65551
  bytes, past the byte cap of 65536 …`
- `a_tracked_context_file_that_is_not_text_is_refused_before_any_case_opens`: red. A case opens, then `metaharness
  refused the session: --context logo.bin is refused: its content is not UTF-8 text without NUL bytes (not UTF-8 after
  byte 0)`
- `scenario_run_checks_context_against_the_tree_the_case_is_cut_from`: red. The case opens and the tests and gates
  run, then step 3: `the context file "later.md" is not one of the files the session's copy holds, spelled as one plain
  relative path`
- `a_flag_wins_over_the_files_budget_and_effort`: green.
- `a_kind_changed_by_the_flag_refuses_the_files_setting_naming_the_file`: green.
- `context_paths_the_copy_does_not_hold_are_refused_naming_them`: green (a tracked link, a path under a linked
  directory, an untracked file, `.git/…`, `a//b`, a trailing `/`).

Suite: `cargo test -p b10x-takt-cli --test w51_a_adversary`: `test result: FAILED. 3 passed; 5 failed; 0 ignored; 0
measured; 0 filtered out; finished in 2.27s`, exit 101.

## Findings

All four are introduced: at d661e73b1 these keys were refused outright as not carried yet.

1. `crates/takt-cli/src/main.rs:688` with `crates/takt/src/run_config.rs:630`, NEEDS-CHANGE, warning. `takt scenario
   run` checks `context` against the repository's current index; the case is cut from the scenario's seed, which can
   lack the file, and Loom refuses the session at step 3 after the case opened, against decision 1. Reached by
   `--context` or the file's `context` on any scenario whose pinned seed predates the file. The reverse (a file the
   seed holds and HEAD does not) is not tested. Fix: check against the seed tree once `scenario_run` has resolved it.
2. `crates/takt/src/run_config.rs:630`, NEEDS-CHANGE, warning. The `context` check does not apply metaharness's launch
   check (at most 65,536 bytes in total, UTF-8 text without NUL), so a long or binary tracked file opens a case that
   can never start a session. Reached by naming any long tracked file. Fix: the same check before any case opens,
   through `metaharness-protocol`'s `context_cost` and `DeclaredContextFile::new`.
3. `crates/takt/src/run_config.rs:716` (and `:675`), CONFIRMED, note. serde_json writes a YAML number below 1e-5 in
   exponent form, which `micro_usd_of` refuses: numbers from 0.000001 to 0.000009 are refused though the reference
   admits a number down to one micro-dollar. The string form works.
4. `crates/takt/src/run_config.rs:611`, CONFIRMED, note. A refusal of the merged settings names every session flag
   given as refused, including ones the harness accepts (`--effort` under codex).

## Attacked and could not break

Budget values (1.5, 0.3 exact; sign, exponent, more than six decimals, overflow, null refused; 0 accepted as
documented); context dot segments, absolute paths, links and untracked files; flag over file for budget and effort; a
kind change by `--harness`; Loom-reserved settings (`SessionOverrides` has four members; effort is its own argv
element for Claude Code and TOML-quoted for Codex).

```findings
[
  {"file": "crates/takt-cli/src/main.rs", "line": 688, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "takt scenario run checks context against the repository's current index, not the seed the case is cut from, so a file added after the seed opens a case that Loom refuses at the harness step (scenario_run_checks_context_against_the_tree_the_case_is_cut_from)"},
  {"file": "crates/takt/src/run_config.rs", "line": 630, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the context check does not apply metaharness's 64 KiB cap or its text check, so a long or binary tracked file opens a case whose session is refused at launch (a_tracked_context_file_over_the_byte_cap_is_refused_before_any_case_opens, a_tracked_context_file_that_is_not_text_is_refused_before_any_case_opens)"},
  {"file": "crates/takt/src/run_config.rs", "line": 716, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "a YAML number budget from 0.000001 to 0.000009 is written by serde_json in exponent form and refused, against the documented one-micro-dollar floor (a_one_micro_dollar_budget_written_as_a_number_reaches_the_session)"},
  {"file": "crates/takt/src/run_config.rs", "line": 611, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "a refusal of the merged settings names every session flag given as refused, including ones the harness accepts (a_refusal_under_codex_names_only_the_flag_codex_does_not_take)"}
]
```
