---
format: aep.planning-md/3
id: story:report-shows-cached-and-uncached-tokens
kind: story
status: draft
title: Report and budget stop show cached and uncached tokens
summary: the four token components beside the total; the budget stays on the total
tags:
- trial-0.24.0
revision: 1
---
## Outcome

`takt report --cases` and the budget-stop message show cached and uncached tokens apart, so an
operator sees what a run cost. The budget itself keeps counting total tokens: a conservative
stop, not a larger spend.

## Context

`TokenUsage::total` (`crates/takt/src/events.rs:51-56`) sums input, output, cache reads and cache
writes; the budget and the scorecard use it. In the 0.24.0 trial cycle one run's 54,595 tokens
were 10 input, 2,928 output, 34,367 cache read and 17,290 cache write, and a cluster trial's steps
were 86-92% cache reads.

## Acceptance

1. `takt report --cases <dir>` prints, per repository and day and in the total, the input, output,
   cache-read and cache-write tokens beside the total; a component no record states is `unknown`.
2. A run stopped by its token budget names the total that crossed it and the same four
   components.
3. The budget still stops on the total of the four components, as in 0.24.0.
