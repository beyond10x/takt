---
format: aep.planning-md/3
id: story:failed-step-tokens-are-recorded
kind: story
status: draft
title: Tokens a failed harness step reported are recorded
tags:
- finding
refs:
- provider: finding
  reference: failed-step-tokens-are-recorded
relations:
- informed_by: review-result:findings-hunt-3fe3f21
- decomposes: epic:recursive-self-improvement
revision: 1
---
## Outcome

Tokens a harness reports are recorded and charged to `--tokens` even when its step fails.

## Context

`ClaudeCode::step` bails (no action line, non-zero exit, cut output) before `self.reported.set(usage)` (`harness.rs:262-263`). The error path in `run_held` (`run.rs:3928-3943`) writes an `error` line with no tokens. The fake harness reported 700+300 tokens; the stop line read `tokens 0` and the scorecard row read `tokens 0`. I don't know whether real `claude -p` exits 0 or non-zero when it hits `--max-turns`; both paths drop the usage. The blob is a fake `claude` that prints that usage and no action line.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCnByaW50ZiAneyJyZXN1bHQiOiJzdG9wcGVkIGJlZm9yZSB0aGUgYWN0aW9uIGxpbmUiLCJ1c2FnZSI6eyJpbnB1dF90b2tlbnMiOjcwMCwib3V0cHV0X3Rva2VucyI6MzAwfX1cbicK | base64 -d > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --forge none --test-cmd ./check.sh --iterations 5
exit: 5
output: the harness ended without the JSON line naming its action; steps 3, tokens 0,
source: .engineering/planning/review-result/findings-hunt-3fe3f21.md
```

## Scope

`crates/takt/src/harness.rs` (`ClaudeCode::step`), `crates/takt/src/run.rs` (harness error path), `crates/takt/src/events.rs` (tokens on `error`).

## Acceptance

In the predicate's run, the stop line and the scorecard show 1000 tokens, and a `--tokens` budget counts them.
