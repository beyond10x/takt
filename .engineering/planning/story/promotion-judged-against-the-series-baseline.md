---
format: aep.planning-md/3
id: story:promotion-judged-against-the-series-baseline
kind: story
status: active
title: takt promote also judges against the first record of the series
summary: without the daily cap, one-case-worse steps can add up
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion-without-daily-cap
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:58Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:58Z", actor: "human:timo", revision: 3}
---
# Story: promotion-judged-against-the-series-baseline

## Outcome

`takt promote` judges a candidate against the latest record and against the first record of the
current promotion series, so steps of "one case worse" cannot add up across back-to-back
promotions.

## Context

story:promotion-without-daily-cap removed the one-promotion-a-day cap so improvement trials run
back to back. The review of that change (2026-10-19) noted that each promotion is judged only
against the one before it, which allows one case worse per scenario per step; the daily cap was
the only limit on how fast that could add up.

## Acceptance

- A chain of three promotions, each one case worse than the one before on one scenario, has its
  third refused, naming the series baseline and the scenario: a test, red today.
- What starts a series is stated in `takt promote --help` and the ESS (for example
  `--rebaseline`, or an owner-named record).
