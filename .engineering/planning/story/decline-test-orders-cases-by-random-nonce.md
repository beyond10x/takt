---
format: aep.planning-md/3
id: story:decline-test-orders-cases-by-random-nonce
kind: story
status: draft
title: A decline test orders two trial cases by ids whose order a random nonce decides
summary: decline_adv1.rs:775 fails when both trials fall in one second
tags:
- finding
- priority:medium
revision: 1
---
## Outcome

`passing_over_a_declined_case_does_not_pass_over_its_storys_other_open_case` (`crates/takt-cli/tests/decline_adv1.rs`) passes whatever trial numbers its two scenario runs draw.

## Context

On pull request 50, a store-only change, CI run 41169055032 failed in `task check (test-cli)` at `decline_adv1.rs:775`: `the declined case is first in case order`. The same check passed on `main` at `8ee32589` (run 41168902726).

The test runs two scenario trials, then asserts `declined < open` on their case ids. In a repository with no `origin`, `unique_trial` (`crates/takt-cli/src/main.rs:3799-3815` at `d016522a`) numbers a trial as the second times 10^8 plus a random nonce. When both runs fall in the same second, which id sorts first depends on the two nonces, so the assertion fails on about half of those runs. This cause is inferred from the code: the log does not print the two ids.

## Scope

`crates/takt-cli/tests/decline_adv1.rs` (the test), or the case order the frontier uses, if the story's design means a creation order rather than the ids' order.

## Acceptance

The test names its cases by the order the frontier actually takes them in, so a random nonce cannot flip that order. It passes when both trials run in the same second, with the second trial's nonce below the first's.
