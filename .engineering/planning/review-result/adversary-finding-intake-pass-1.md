---
format: aep.planning-md/3
id: review-result:adversary-finding-intake-pass-1
kind: review-result
status: active
title: Adversary, finding-intake, pass 1
relations:
- reviews: story:finding-intake
revision: 1
---
unit: story:finding-intake, worktree w8-finding-intake at c469442 (base 4cbc096) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 332→341, red 9
origin: introduced 9 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, the unit's scratch `adv1/`
needs-coordinator: none

**1. Diff.** `git --no-pager diff --stat` is empty; `git status --short` shows `?? crates/takt-cli/tests/finding_scripts_adv1.rs`. No script, doc or store write.

**2. Cases added** (`crates/takt-cli/tests/finding_scripts_adv1.rs`, all red; alone: `FAILED. 0 passed; 9 failed`)

| test | expected | actual |
|---|---|---|
| `a_scenario_declared_only_through_an_include_gives_no_result` | exit 1, empty stdout (takt's reader uses `--no-includes`, `crates/takt/src/git.rs:204`) | `exit Some(0)` `stdout: reproduced` |
| `a_scenario_include_naming_a_fifo_does_not_hang_the_check` | finishes; timeout 5 | `check-finding still ran 20 s after it started, blocked on the declaration's include` |
| `the_predicate_reaches_no_credential_under_home` | `not_reproduced` | `exit 0; reproduced`: `git credential fill` returned the password planted in the HOME credential store |
| `a_scenario_checkout_left_on_trial_0_still_gives_a_result` | `reproduced` | `fatal: a branch named 'trial/0' already exists` / `check out the seed 1ee6ded55d89; no result` |
| `a_relative_body_is_filed_from_a_subdirectory_of_the_repository` | `filed` | `error: reading the body from finding.md: No such file or directory` / exit 1 |
| `the_body_filed_is_the_body_whose_predicate_was_checked` | refused, or the filed body holds `expected_before` | filed without a predicate |
| `two_filings_of_one_key_a_moment_apart_leave_one_artifact` | 1 artifact | `left: 2 right: 1` (`blocker:` and `story:` of one key, both `filed`) |
| `a_predicate_expecting_exit_124_is_checked_or_refused_at_filing` | `reproduced`, or `malformed` at filing | filed; the check exits 1, empty stdout |
| `a_timeout_that_is_no_duration_gives_no_result` | non-zero, empty stdout | `exit 125`; `not_reproduced` on stdout, exit 0 |

**3. Gate.** fmt exit 0; `cargo test --workspace --locked --no-fail-fast` exit 101, 341 executed, 332 passed, 9 failed, only `finding_scripts_adv1`; `ess specify validate --path ess` exit 0, `takt v1 — 2 file(s), valid`.

**4. Attacked, not broken.** aep 0.74.0 accepts a second artifact with the same `--ref` (exit 0) and refuses the same id in one kind; `list --ref` matches exactly, archived included. Key and scenario regexes, source `..` and realpath checks, multiple `expected_before` blocks, CRLF: held. Every failure path printed nothing on stdout except finding 8. Build cache keyed by full sha, copy then rename, EXIT trap on TERM/INT/HUP, tags through `^{commit}`, dirty tree ignored by `git archive`. Ten simultaneous filings with the real aep left no duplicate (aep's writer lock). New `read from:` citations resolve (checked by hand).

```findings
[
  {"file": "scripts/check-finding", "line": 110, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the scenario declaration is read with includes followed, unlike takt scenario run's --no-includes reader, so a scenario takt refuses yields a result and a FIFO include hangs the check outside its timeout"},
  {"file": "scripts/check-finding", "line": 150, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the predicate gets the caller's HOME, so stored git credentials and the operator's harness login reach it, contrary to the header's no credential of the caller reaches it"},
  {"file": "scripts/check-finding", "line": 142, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a scenario checkout left on trial/0 by takt scenario run makes the clone's checkout -b trial/0 fail, so the documented reproduce step gives no result"},
  {"file": "scripts/file-finding", "line": 171, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a relative --from is checked from the caller's directory but read by aep after cd to the repository, so filing from a subdirectory fails"},
  {"file": "scripts/file-finding", "line": 147, "category": "acceptance", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "with a same-named file at the repository root, the body filed is not the body whose predicate was parsed, so a finding without expected_before is filed"},
  {"file": "scripts/file-finding", "line": 163, "category": "concurrency", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the duplicate check and the write are not atomic, so two filings of one key under different kinds can both be filed"},
  {"file": "scripts/check-finding", "line": 155, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a predicate expecting exit 124 or 137 is accepted at filing but always reads as a timeout and never gives a result"},
  {"file": "scripts/check-finding", "line": 155, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "an invalid CHECK_FINDING_TIMEOUT makes timeout exit 125 without running the command, and the script prints not_reproduced instead of no result"},
  {"file": ".agents/operator-agent.md", "line": 103, "category": "judgement", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Intake step 4 is garbled and does not say which revision to pass"}
]
```
