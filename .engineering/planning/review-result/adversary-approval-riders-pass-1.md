---
format: aep.planning-md/3
id: review-result:adversary-approval-riders-pass-1
kind: review-result
status: active
title: Adversary, approval-riders-are-grants, pass 1
relations:
- reviews: story:approval-riders-are-grants
revision: 1
---
unit: story:approval-riders-are-grants, worktree w9-approval-riders at bd31a66 (base 8ce2019) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 366→372, red 1
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 3 paths, all in the unit's scratch `adv1/`
needs-coordinator: none

**Cases** (`crates/takt/tests/approval_riders_adv1.rs`)

| case | expected | now |
|---|---|---|
| `adv1_a_record_that_also_answers_a_review_does_not_ride` (the real record plus `change.review` and `change.outcome: fixed`) | does not ride | red: rides |
| `adv1_real_approvals_by_unusual_grantors_ride` (`Jane  Doe`, ` lead `, combining marks) | rides | green |
| `adv1_a_record_at_the_cap_rides_and_one_byte_above_it_does_not` (65536 / 65537 bytes) | rides / refused | green |
| `adv1_look_alikes_and_duplicate_keys_do_not_ride` (`finding:` namespace, zero-width space, capital, Cyrillic о, `.000Z` and `+00:00` instants, duplicate `kind` both orders) | refused | green |
| `adv1_a_symlink_holding_the_record_and_a_gitlink_do_not_ride` (mode 120000 with the record as target text; 160000) | refused | green |
| `adv1_a_grant_denied_or_rebound_after_its_record_does_not_ride` | refused, refused, rides | green |

Red alone: `panicked at crates/takt/tests/approval_riders_adv1.rs:283:5: an approval record that also records a review outcome rides on de976af6…`.

**Gate.** fmt 0, clippy 0; `cargo test --workspace --locked --no-fail-fast` exit 101, 371 passed, 1 failed, only `approval_riders_adv1` (5 passed; 1 failed); `approval_riders` 5/5, `approval_record` 6/6, `approval_record_adv2` 5/5.

**Runtime check of what reaches the finding.** In a scratch store, the forged rider's `review`/`outcome` were read by the store whatever the kind: `aep plan artifact history` printed `approval recorded from takt approve … : review-result:adv1 was fixed`, and `review-value` went from `"fixed": 0` to `"fixed": 1`. The store gates moves on counts by kind, and the kind stays `approval`, so it falsifies a review record without admitting a move.

**Attacked, not broken.** Duplicate named fields refused by serde; different instant, look-alike story or grantor, another namespace refused by exact comparison; a grant from another revision, denied or replaced refused; one grant recorded twice, in one commit or two, refused; the 64 KiB cap exact; symlink, gitlink, subdirectory, rename, mode change refused; the subject-capability rule matches `approve_subject` (`frontier.rs:264`); the rewritten tests weakened no assertion. Not tested: two capabilities at one revision (the protocol has only `repository.merge`). Changing `actor`, `revision` or the top-level `kind` also rides; no store reader found that gates on them.

```findings
[
  {"file": "crates/takt/src/case.rs", "line": 421, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "StoreRecord/StoreChange ignore unnamed fields, so a rider holding the grant's real record plus change.review/change.outcome rides, and the store reads it as that review's outcome (review-value fixed 0 to 1)"},
  {"file": "crates/takt/tests/approval_record_adv2.rs", "line": 337, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the symlink variant's target is a file path, not JSON, so the new text check refuses it and the mode check it claims to exercise is no longer what decides; approval_riders_adv1 adds a symlink holding the record"}
]
```
