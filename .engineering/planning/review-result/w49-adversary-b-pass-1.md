---
format: aep.planning-md/3
id: review-result:w49-adversary-b-pass-1
kind: review-result
status: active
title: 'Wave 49 adversary pass 1 on unit b: the forge wait backoff'
relations:
- reviews: story:ci-wait-watches-the-check
revision: 1
---
# Adversary pass 1 on unit b (wave 49)

```
unit: story:ci-wait-watches-the-check (unit b), tree takt-w49-b at dc3040c64 plus one untracked test file
verdict: green
cases: executed 18→22, red 0
origin: introduced 2, pre-existing 1, undecided 0
wrote-outside-worktree: none
needs-coordinator: yes
```

No case failed. Three findings, none blocks the unit; finding 1 needs a decision: the decided 15 s
cap and the story's "under 5 s" cannot both hold for a CI that runs longer than about 9.5 s.

Cases added in `crates/takt/tests/w49_adv_ci_wait.rs`, all green:

- `a_second_run_on_the_case_starts_the_backoff_at_the_first_interval` (:341)
- `the_default_backoff_is_cut_to_a_short_hour_budget_and_printed_as_recorded` (:378)
- `a_forge_error_between_waits_ends_the_run_and_the_next_starts_the_backoff_again` (:433)
- `a_tiny_poll_still_grows_and_is_capped` (:473)

Commands: `cargo test -p b10x-takt --test w49_adv_ci_wait`: `ok. 4 passed`;
`--test ci_wait_backoff --test gitlab_ci_wait --test ci_pending_waits`: `ok. 10 passed` /
`ok. 5 passed` / `ok. 3 passed`.

Findings:

1. 15 s cap vs the 5 s acceptance (`crates/takt/src/run.rs:9587`; introduced, CONFIRMED, warning).
   Under the default backoff the reads after the first pending one fall at 2, 5, 9.5, 16.25 and
   26.4 s; a CI finishing between 9.5 and 11.25 s, or between 16.25 and 21.4 s, gets its
   `ci_result` more than 5 s later.
2. Out of scope: step budgets cover less CI time (`run.rs:11913`; introduced, CONFIRMED, note).
   Every forge read is a step, so `--iterations N` runs out after far less CI wall time.
3. Zero poll (`run.rs:10914`; pre-existing, INFEASIBLE, note). As the base's `pause()` did;
   nothing outside tests sets `poll`.

Attacked without breaking: reset never or always; the cap exceeded; a wait longer than the time
left; a resumed run from the cap; the head changing while checks stay pending; a forge error
between waits; recorded seconds vs printed line; `Run::POLL` gone. ESS
(`ess/domains/takt.yaml:2668`) matches the code.

```findings
[
{"file": "crates/takt/src/run.rs", "line": 9587, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "With the 15 s cap the default reads fall at 2, 5, 9.5, 16.25 and 26.4 s, so a CI finishing in 9.5-11.25 s or 16.25-21.4 s gets its ci_result more than 5 s late, against the acceptance's bound; no test, the live measurement is a trial"},
{"file": "crates/takt/src/run.rs", "line": 11913, "category": "out-of-scope", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "Every forge read is a step, so under the backoff a --iterations budget runs out after far less CI wall time than under the old fixed 30 s poll"},
{"file": "crates/takt/src/run.rs", "line": 10914, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "A zero Run::poll makes every wait 0 s and reads the forge back to back until the budget ends, as the base's pause() did; nothing outside tests sets poll"}
]
```
