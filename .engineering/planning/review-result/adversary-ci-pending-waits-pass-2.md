---
format: aep.planning-md/3
id: review-result:adversary-ci-pending-waits-pass-2
kind: review-result
status: active
title: Adversary, ci-pending-waits, pass 2
relations:
- reviews: story:ci-pending-waits
revision: 1
---
unit: story:ci-pending-waits, worktree w8-ci-pending-waits at 0e21f3d (base 4cbc096) plus one untracked test file
verdict: INFEASIBLE
cases: executed 337→341, red 2
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, the unit's scratch `adv2/` (logs and a mutant copy of the tree)
needs-coordinator: none

**1. Diff.** `git --no-pager diff --stat` is empty; `git status --short` shows `?? crates/takt/tests/ci_pending_waits_adv2.rs`.

**2. Cases added** (alone: 2 passed, 2 failed)

| test | expected | actual |
|---|---|---|
| `adv2_the_words_b10x_gates_prints_for_a_405_are_classified_as_the_forge_meant_them` | b10x-gates 0.1.21's real stderr (`ApiFailure::render`) arrives as `Failed`; pending waited on, failing or withheld stays an error | green |
| `adv2_a_pending_check_whose_name_holds_a_failing_phrase_is_waited_on` | pending check named `"e2e has failed before"` is waited on | green; kills the mutant `if false && c == '"'` at forge.rs:152 |
| `adv2_a_failing_check_whose_name_ends_in_a_quote_does_not_turn_a_mixed_refusal_into_a_wait` | `…"build" is expected.; Required status check "lint"" is failing.` is an error | red: `a refusal naming a failing required check is waited on, not an error` (:127) |
| `adv2_a_refusal_that_also_requires_a_review_is_not_waited_on` | a ruleset 405 requiring an approving review besides a pending check is an error | red: `a refusal that needs a review as well is waited on` (:142) |

**3. Gate.** fmt 0, clippy 0; `cargo test --workspace --locked --no-fail-fast` exit 101, 339 passed, 2 failed, only `ci_pending_waits_adv2`; `ess specify validate --path ess` 0.

**4. Attacked, not broken.** Every bot-route merge path yields `Failed`; `reading_of` never produces `exited N: `. b10x-gates 0.1.21 puts the forge's message after its prefix and the classifier reads it; "message withheld" stays an error. Display text unchanged. No known GitHub 405 other than a required check contains `required status check`. `--hours` `-0`, `0`, `NaN`, `inf` refused with exit 1; `1e-300`, `1e16`, `1.7e308` accepted without panic; library NaN stops `Hours (NaN)`. The 5 new `read from:` lines resolve. Help line, README row 6 and the printed stop agree. Budgets, no judgement step while waiting and no record while waiting unchanged; no existing assertion changed.

```findings
[
  {"file": "crates/takt/src/forge.rs", "line": 152, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the quote toggle assumes balanced quotes, so a failing check whose name ends in a quote hides its own is failing and a mixed pending and failing refusal is waited on instead of erroring"},
  {"file": "crates/takt/src/forge.rs", "line": 159, "category": "acceptance", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a ruleset refusal naming a required review besides a pending check is read as a pending-check wait, so the run spends its budget and exits 6 instead of reporting the review"}
]
```
