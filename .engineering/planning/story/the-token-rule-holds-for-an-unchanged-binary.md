---
format: aep.planning-md/3
id: story:the-token-rule-holds-for-an-unchanged-binary
kind: story
status: active
title: The token rule does not refuse a candidate whose tokens per request equal the baseline's
summary: 0.2.2 refused on tokens with byte-identical prompts; baseline max below today's median
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T16:18:10Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T16:18:10Z", actor: "human:timo", revision: 3}
---
# Story: the-token-rule-holds-for-an-unchanged-binary

## Outcome

`takt promote` and `scorecard.compare` do not refuse a candidate whose tokens per request equal the
baseline's: the token rule tolerates the model's run-to-run variance and drift outside takt
(Claude Code's own tool set), and still refuses a real per-request regression.

## Context

On 2026-10-19 the promotion judge refused takt 0.2.2 (f0728964) against 44f96a48 with `median
tokens 229309 exceeds the saved maximum 122058`. The RSI token investigation found both binaries
send byte-identical step prompts; ten interleaved live runs give medians of 122,355 (44f96a48)
and 122,415 (f0728964) over the first three Claude steps, already above the saved maximum of
122,058, because Claude Code's own tool set grew by 268 to 570 tokens a request since the
baseline was measured. A step that makes one more API request than usual costs about 16.7k more;
that happened in 15 of 55 `implementation.edit` steps. The saved baseline drew 0 of 5, so its
spread is 563 tokens and its maximum sits below the median of an identical binary today. The
investigation estimates an identical binary is refused about 12 % of the time on the API-request
draw alone (inferred), and always while the external drift holds.

## Acceptance

- A candidate identical to the baseline, measured with today's Claude Code, is not refused on
  tokens: a test over recorded scorecards (the 0.2.2 and 44f96a48 cases are kept outside the
  repository).
- A candidate whose every request costs 10 % more than the baseline's is refused: a test.
- The rule and its tolerance are stated in `takt promote --help` and the ESS, and the design's
  open question on the token rule is closed.
