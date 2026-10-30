---
format: aep.planning-md/3
id: story:a-merge-read-merged-after-unknown-completes-in-one-process
kind: story
status: draft
title: A merge read merged after outcome_unknown completes in the same process
summary: read_again lets settle_unknown count it applied; no merge sent
tags:
- known-limit
- priority:low
- release-0.25.0
relations:
- informed_by: story:connectors-forge-at-a-served-instance
revision: 1
---
## Outcome

Within one process, a merge answered `outcome_unknown` whose pull request then reads merged at
the head named completes as applied. Today it is refused by the read that precedes the merge
until a resume settles it.

## Context

Known limit of 0.25.0, from the wave 48 adversary pass on unit d (review-result
`wave-48-adversary-d`, INFEASIBLE, origin undecided). In the same process, `read_again`
(`crates/takt/src/forge/connectors.rs`, around line 1324 at 0.25.0) refuses a pull request read
merged after an unknown merge. That happens before `settle_unknown` can count it as applied.

No caller merges again in the same process: the validator stops on the error, and a resume
settles it. So the case
`in_one_process_an_unknown_merge_read_merged_completes_with_no_second_merge`
(`crates/takt/tests/w48_adv_d.rs:1286`) is ignored with that reason.

## Acceptance

1. In one process, after a merge answered `outcome_unknown`, a second merge call on a pull
   request read merged at the head named returns applied. No merge is sent.
2. The ignored case at `w48_adv_d.rs:1286` runs and passes, and its `#[ignore]` is removed.
3. A pull request read merged at another head, closed or locked still sends no merge.
