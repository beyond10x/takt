---
format: aep.planning-md/3
id: epic:benchmark-integration
kind: epic
status: draft
title: 'Benchmark integration: tasks agents can fail, under four arms'
relations:
- derived_from: design:benchmark-integration
- serves: vision:safe-agentic-coding
revision: 1
---
# Epic: benchmark-integration

## Outcome

takt is measured on tasks that agents can fail: six realistic scenarios of its own first, then pinned subsets of Terminal-Bench 2.0 and SWE-bench Verified Mini, each under the four arms of the baseline method, with the benchmark tests held out, every figure raw, and every run exported to the organization's benchmark instrument.

## Context

design:benchmark-integration. Owner request of 2026-10-18: "we need a bit more realistic use-cases ... we first work on our own trial/benchmark but later we do want to have it".

## Acceptance

docs/metrics holds one comparison per stage: six scenarios and one pinned subset of each benchmark, each under four arms, with verdicts from held-out tests and raw usage, and the same runs are records in the organization's benchmark instrument.
