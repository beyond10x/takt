---
format: aep.planning-md/3
id: story:step-transcripts-are-kept
kind: story
status: draft
title: Each harness step's stream is kept under the case's artifacts
summary: trial 1 needed a recording shim to read why a run looped
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
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

- After a run with the claude harness, `.engineering/cases/<id>/artifacts/step-<n>.jsonl` exists
  for every harness step and holds the stream the harness printed: a test with a fake harness.
- A step that errors still keeps its transcript.
- The transcript is not on the judge surface's read path: no validator reads it.
