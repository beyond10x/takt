---
format: aep.planning-md/3
id: review-result:adversary-efficiency-pass-2
kind: review-result
status: active
title: Adversary, efficiency unit, pass 2
relations:
- reviews: story:pr-opens-when-change-is-done
revision: 1
---
# Adversary pass 2: the efficiency unit

Unit at 45ad4cb (impl/efficiency; unit 22a8389, correction 1 45ad4cb). Verdict: NEEDS-CHANGE, 4 red
cases in `crates/takt/tests/efficiency_adv2.rs`; 535 → 539 executed.

Attacked without a break: change.read due more than once per hand-over; change_record written as a
choice; commits the step guard put back; store-only or cases-only commits; a missing or unpinned
base; `--harness none` looping at the seed; the approval stop with input-blocked producers; every
protocol on disk (only `handover` is a choice kind, none refused).

```findings
[
  {
    "file": "crates/takt/src/validator.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "change.read reads ahead for a revision whose tree is the base, so a pull request with an empty diff is opened: the harness edits README.md, writes it back as it was, hands over; change_record ahead, pr.open opens #1, CI passes, the run stops ApprovalRequired with nothing changed",
    "line": 1382
  },
  {
    "file": "crates/takt/src/validator.rs",
    "category": "correctness",
    "severity": "note",
    "message": "an edit whose summary carries a line 'takt: take <x>' is not counted as a change: implementation.edit with summary 'fixed the story\\ntakt: take story:anything' then a hand-over; change_record none, no pull request, NoProgress",
    "line": 1378
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "a choice record makes tests.pass true when its action is not named tests.run: a protocol reads test_result without a result, produced by tests.check (effect read); takt.choice writes test_result and the case completes accepted on it alone",
    "line": 287
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "a choice kind read about no artifact is refused only after the case is opened: Run::open creates the case directory and checks out takt/<id>, then Run::run refuses the protocol; the case is left open on its branch",
    "line": 1452
  },
  {
    "file": "crates/takt/tests/pr_opens_when_done.rs",
    "category": "correctness",
    "severity": "note",
    "message": "the BUILT_IN_ACTIONS drift test lists the validators by hand and cannot catch a validator left out: a seventh validator added to validator.rs without updating BUILT_IN_ACTIONS or the test leaves the test green",
    "line": 517
  }
]
```
