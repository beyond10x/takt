---
format: aep.planning-md/3
id: story:a-no-action-retry-is-not-a-full-session
kind: story
status: draft
title: A retry after no action is not a full-price session
summary: retries ask only for the missing action, and their tokens are recorded
tags:
- trial-0.24.0
revision: 1
---
## Outcome

A step whose session names no action is not paid for three times over: a retry costs a small
fraction of a fresh session, or the run stops sooner, and the scorecard says what the retries
cost.

## Context

In a cluster trial run, steps whose session named no action took about 3.0M of the run's
5,185,629 tokens (58%); the steps that named one took 2.17M. `NO_ACTION_TRIES = 3`
(`crates/takt/src/run.rs:8731`, the loop at `run.rs:1032-1068`) starts each retry as a fresh
session over the whole context. Whether the action line was present and missed is not known; the
transcripts were not kept.

## Acceptance

1. A retry after "the session named no action" asks only for the missing action, in the same
   session or with the previous answer as context, not the whole step again.
2. With a harness stand-in that never names an action, the run stops having spent at most 1.5
   times one session's tokens on that step (today 3 times); the stop line names the step and the
   tries.
3. Each try's tokens are recorded in the run's events, so the cost of retries can be read from
   `takt report`.
