---
format: aep.planning-md/3
id: review-result:adversary-approval-riders-pass-2
kind: review-result
status: active
title: Adversary, approval-riders-are-grants, pass 2
relations:
- reviews: story:approval-riders-are-grants
revision: 1
---
unit: story:approval-riders-are-grants, worktree w9-approval-riders at 63f5d6e (base 8ce2019) plus one untracked test file
verdict: INFEASIBLE
cases: executed 372→382, red 2
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 19 paths, all in the unit's scratch `adv2/`
needs-coordinator: none

**Cases** (`crates/takt/tests/approval_riders_adv2.rs`, each approving through the real aep 0.74.0; alone: `8 passed; 2 failed`)

| case | expected | now |
|---|---|---|
| `adv2_a_real_approval_rides_and_is_merged` | rides, `Completed("accepted")`, merged once | green |
| `adv2_the_real_record_has_exactly_the_fields_the_rider_check_requires` | six top-level fields, four `change` fields, revision = story's | green |
| `adv2_a_story_saved_with_crlf_still_carries_its_real_approval` | rides (aep reads CRLF front matter) | red: refused |
| `adv2_every_revision_spelling_the_store_reads_carries_its_real_approval` | rides | green |
| `adv2_a_revision_in_the_story_body_is_not_read` | real rides, forged not | green |
| `adv2_a_story_the_store_refuses_carries_no_rider` | refused | green |
| `adv2_a_story_with_whitespace_before_its_front_matter_carries_no_rider` | refused (aep cannot open it) | red: rides |
| `adv2_a_rider_that_changes_the_story_does_not_ride` | refused | green |
| `adv2_the_revision_itself_rides_with_or_without_grants_or_a_story` | true | green |
| `adv2_actors_ride_by_the_store_shape` | the store's shape rides, the rest not | green |

**Gate.** fmt 0, clippy 0; `cargo test --workspace --locked --no-fail-fast` 101, 380 passed, 2 failed (the two above). `approval_riders` 5/5, `approval_riders_adv1` 6/6, `approval_record` 6/6, `approval_record_adv2` 5/5, `forge` 35/35; no assertion weakened.

**Not holes.** No aep store reader gates on an evidence record's actor (`journal.rs:43-45` free text; `:450-458` counts by kind; `planning.rs:5653-5671` reads only `review` and `outcome`; `:8046` prints it); takt's actor shape equals aep's accepted characters. A real aep 0.74.0 approve writes exactly the fields takt requires; `deny_unknown_fields` gives no false refusal, and the fields test pins the contract. The early return behaves as the old loop did.

```findings
[
  {"file": "crates/takt/src/case.rs", "line": 492, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "story_revision_at needs an LF front matter, so on a story committed with CRLF front matter, which aep reads, the store's own approval record is refused and the case can never merge"},
  {"file": "crates/takt/src/case.rs", "line": 490, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the story blob is read through git::run, which trims, so takt reads a revision from a story with whitespace before its opening fence that aep refuses to open"}
]
```
