---
format: aep.planning-md/3
id: story:step-transcripts-are-kept
kind: story
status: active
title: Each harness step's stream is kept under the case's artifacts
summary: trial 1 needed a recording shim to read why a run looped
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:59:00Z", actor: "human:timo", revision: 3}
---
# Story: step-transcripts-are-kept

## Outcome

Each harness step's stream (prompt, tool calls, replies, usage) is kept under the case's
`artifacts/`, named by step, so an improvement turn can be read after the run.

## Context

takt runs Claude with `--output-format json --no-session-persistence`
(`crates/takt/src/harness.rs:266-279`) and keeps no transcript. RSI trial 1 had to put a recording
shim on `PATH` to read why the delivery run looped (story:delivery-refuses-an-obligation-nobody-performs).
Reading transcripts is how improvement rounds find their next finding.

## Acceptance

- After a run with the claude harness,
  `$XDG_STATE_HOME/takt/transcripts/<repo-key>/<case-id>/run-<r>/step-<n>.jsonl` exists for every
  harness step and holds the stream the harness printed: a test with a fake harness. Outside the
  cases directory, because the cases snapshot holds every file there in memory (planner,
  2026-10-19, after the review's 700 MB measurement).
- A step that errors still keeps its transcript.
- The transcript is not on the judge surface's read path: no validator reads it.
