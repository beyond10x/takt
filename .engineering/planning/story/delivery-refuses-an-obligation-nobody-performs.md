---
format: aep.planning-md/3
id: story:delivery-refuses-an-obligation-nobody-performs
kind: story
status: draft
title: A case stops when its next obligation has no performer, instead of looping
summary: 'trial 1: 457k of 539k tokens looped on pr.open needing gate.green with no gates document'
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: delivery-refuses-an-obligation-nobody-performs

## Outcome

A case whose next obligation can only be met by an action no validator or step performs stops at
once, naming the obligation and the missing performer, instead of looping until NoProgress.

## Context

RSI trial 1 (2026-10-19, scratch state): a `story.delivery/1` run on takt itself landed the fix
at step 2 (82k tokens), then looped from step 3 to step 20 between handover, `change.read` and
re-wrapping a comment: 457k of the run's 539k tokens (85 %). `pr.open` needs `gate.green`, takt
has no `.engineering/gates.yaml`, so no gate validator is registered (`crates/takt-cli/src/main.rs:892`)
and nothing could ever produce `gate.green`; the model was offered only edit, handover and
tests.write. Evidence: the trial's `logs/transcripts-*-analysis.txt`, kept outside the repository.

## Acceptance

- A case whose protocol needs `gate.green` before `pr.open`, run with no gates document, stops
  before any model step with a refusal naming `gate.green` and the missing gates document (exit 1
  at open, or exit 3 at the first step that cannot advance): a test, red today.
- A case with a gates document is unchanged.
