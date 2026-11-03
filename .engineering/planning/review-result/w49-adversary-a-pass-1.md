---
format: aep.planning-md/3
id: review-result:w49-adversary-a-pass-1
kind: review-result
status: active
title: 'Wave 49 adversary pass 1 on unit a: the gate deferral'
relations:
- reviews: story:story-delivery-tests-first
revision: 1
---
# Adversary pass 1 on unit a (wave 49)

unit: a (story:story-delivery-tests-first), tree takt-w49-a at fa89f8169 (unit commit 4ccf8055b) plus one new untracked test file
verdict: red
cases: executed 11→17, red 2
origin: introduced 2, pre-existing 0, undecided 0
wrote-outside-worktree: none
needs-coordinator: yes (pick how to fix F1: change the wording or change the behaviour)

Cases added in `crates/takt/tests/w49_adv_gate_deferral.rs`:

- `adv_a_no_revision_the_gate_ran_at_is_recorded_as_deferred`: red. The printed log shows `gate.run: deferred (precondition change.handed_over, tests.pass not true at f3114aa2)` and then `step 7: gate.run {}` passing at the same revision.
- `adv_a_an_unreadable_record_line_does_not_say_a_deferral_twice`: red. A revision deferred 2 times across a resume over an unreadable line.
- Green: `adv_a_a_case_resumed_step_by_step_says_each_deferral_once`, `adv_a_each_revision_runs_the_gate_everywhere_and_defers_nothing`, `adv_a_the_printed_deferral_is_the_recorded_one_and_its_ess_shape`, `adv_a_the_trace_holds_one_gate_op_and_no_deferral_as_a_pass`.

Suite: `cargo test -p b10x-takt --test w49_adv_gate_deferral`: `test result: FAILED. 4 passed; 2 failed`; `--test gate_deferral`: `ok. 1 passed`; `--test protocol_settings`: `ok. 10 passed`.

| # | file:line | verdict | origin | finding | what reaches it |
|---|---|---|---|---|---|
| F1 | crates/takt/src/run.rs:11422 | NEEDS-CHANGE | introduced | The handed-over revision also gets a `gate.deferred` line and a printed deferral, and then the gate runs there; `events.rs:255`, the ESS `takt.work.RunGateDeferred` and `events-jsonl.md` say a deferral means the gate did not run at that revision. | Every default `story.delivery/3` story run; scenario 1 itself. |
| F2 | crates/takt/src/run.rs:12450 | INFEASIBLE | introduced | An unreadable middle line of `events.jsonl` starts the run with no record of past deferrals, so a resumed run records the same revision's deferral again. | Nothing found; the line was built by the test, and a torn last line is repaired before the loop. |

Attacked and not broken: a deferral counted as a pass (trace, scorecard, report, `gate_results`); exactly one `gate_result`; resumed runs; `each_revision`; printed line equals recorded line; ESS shape; no deferral written as evidence.


```findings
[
  {"file": "crates/takt/src/run.rs", "line": 11422, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The handed-over revision gets a gate.deferred line and a printed deferral, then the gate runs there, which contradicts 'the gate did not run at revision' in events.rs:255, the ESS takt.work.RunGateDeferred and events-jsonl.md (test: crates/takt/tests/w49_adv_gate_deferral.rs::adv_a_no_revision_the_gate_ran_at_is_recorded_as_deferred)"},
  {"file": "crates/takt/src/run.rs", "line": 12450, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "An unreadable middle line in events.jsonl makes unwrap_or_default() start the loop with no record of past deferrals, so a resumed run records and prints a revision's deferral twice before ending on the read error; nothing found that reaches this state (test: crates/takt/tests/w49_adv_gate_deferral.rs::adv_a_an_unreadable_record_line_does_not_say_a_deferral_twice)"}
]
```
