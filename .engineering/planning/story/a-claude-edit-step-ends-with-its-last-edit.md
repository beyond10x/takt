---
format: aep.planning-md/3
id: story:a-claude-edit-step-ends-with-its-last-edit
kind: story
status: draft
title: A Claude edit step ends with its last edit, not an extra reply request
summary: the action line comes with the final tool call
tags:
- finding
- priority:medium
- trial-0.24.0
relations:
- informed_by: story:codex-edit-step-ends-in-one-request
revision: 1
---
## Outcome

A Claude edit step takes its action line from the same response as its last edit, so it does not
spend one more full request only to write its closing reply.

## Context

From the 0.24.0 trial cycle's token review of 83 step transcripts over 30 runs: 54 steps end with
a request that calls no tool and only writes the prose and the JSON line. These requests cost
643,843 of 1,876,697 tokens (34%), 576,661 of them cache reads; on scenario 1, 219,214 of 543,461
(40%), about 21.9k tokens a run. Example: one scenario-1 step makes an Edit request of 9,338
tokens, then a text-only request of 10,390. The same shape appears in every `tests.write` and
`implementation.edit` step.

Where, at 0.24.0: the prompt asks the model to "end your reply with exactly one line of JSON"
(`crates/takt/src/harness.rs:2973-2983`), and Claude Code ends its turn only on a reply with no
tool call; `harness.rs:1002` sets `max_turns` to 40. story:codex-edit-step-ends-in-one-request
(draft, Codex only) was rejected because takt would apply patch text; no story covers Claude, and
here takt applies no patch text itself.

```expected_before
command: in every scenario-1 transcript step-3.jsonl and step-6.jsonl, read the last distinct assistant message id
output: it holds no tool_use and has cache_read_input_tokens > 0 (20 of 20 steps at 0.24.0)
```

## Acceptance

1. The model may write the JSON action line beside its final tool call. takt ends the session
   after that tool's result (a post-tool `continue: false` through the harness, or accepting a
   turn limit once a valid line has been seen) and reads the line from there.
2. On scenario 1 the last assistant message of a Claude edit step holds a tool call in at least
   18 of 20 steps, and the run's tokens fall by at least 15% against 0.24.0's baseline.
3. A step whose last response holds no valid line falls back to today's closing request; the
   action and its validation are unchanged.
