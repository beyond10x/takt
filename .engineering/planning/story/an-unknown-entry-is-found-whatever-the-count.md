---
format: aep.planning-md/3
id: story:an-unknown-entry-is-found-whatever-the-count
kind: story
status: draft
title: An unknown merge key is found whatever the refused count says
summary: a record whose count lags its key chain sends no key twice
tags:
- known-limit
- priority:low
- release-0.25.0
relations:
- informed_by: story:connectors-forge-at-a-served-instance
revision: 1
---
## Outcome

The record of keyed writes is read whole: an `unknown` entry is found whatever the refused count
says, so a record whose count lags its chain of keys never sends the first key again.

## Context

Known limit of 0.25.0, from the wave 48 adversary pass on unit d (review-result
`wave-48-adversary-d`, INFEASIBLE). `settle_unknown` looks up the `unknown` entry only under the
key that `refused` gives (`crates/takt/src/forge/connectors.rs`, around line 1160 at 0.25.0). A
record whose count lags the keys it names would resend the first key.

No writer makes such a record: both maps are written in one atomic file. So the case
`a_record_whose_count_does_not_reach_its_fresh_key_sends_no_key_twice`
(`crates/takt/tests/w48_adv_d.rs:1213`) is ignored with that reason.

## Acceptance

1. Given a record whose `unknown` chain runs past the key its `refused` count gives, the next
   merge sends no key that the record already names.
2. The ignored case at `w48_adv_d.rs:1213` runs and passes, and its `#[ignore]` is removed.
3. A record whose chain and count contradict each other in a way no order of writes can produce
   is an error that names the file, and nothing is sent.
