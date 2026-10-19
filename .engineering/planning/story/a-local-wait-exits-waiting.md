---
format: aep.planning-md/3
id: story:a-local-wait-exits-waiting
kind: story
status: active
title: A wait on a local file exits 6 at once instead of polling
summary: 'trial 2: 35 polls, 70 decision records in 17 min'
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 3}
---
# Story: a-local-wait-exits-waiting

## Outcome

When a validator waits on something local (a scorecard file under `$XDG_STATE_HOME`), the run
exits 6 (Waiting) at once, naming what it waits for, instead of polling every 30 s with a decision
record per poll.

## Context

RSI trial 2: `scorecard.compare` waited 17 minutes for the candidate's scorecard and polled 35
times, writing 70 decision records (628 KB) to the case; its message said "on the forge" under
`--forge none` (`run.rs:7128`). Forge waits keep their polling.

## Acceptance

- A turn whose `scorecard.compare` finds no candidate scorecard exits 6 after one step, naming the
  file it waits for; no further decision is written: a test.
- Under `--forge none` no message says "forge".
