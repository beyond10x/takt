---
format: aep.planning-md/3
id: story:the-current-claude-code-is-not-qualified
kind: story
status: draft
title: The current Claude Code is not a qualified harness binary
summary: metaharness 0.23.0 qualifies Claude Code 2.1.304, 2.1.305 and 2.1.309; 2.1.312 stops the default harness HarnessUnqualified before its first step
tags:
- finding
- priority:high
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# The current Claude Code is not a qualified harness binary

## What is wrong

takt asks metaharness on open and on every resume whether the harness binary qualifies
(`crates/takt/src/harness.rs`, `qualify`), and stops `HarnessUnqualified`, exit 1, when it does
not (`crates/takt/src/run.rs`, `Stop::HarnessUnqualified`). metaharness 0.23.0, which the loom
0.49.1 pin carries, qualifies Claude Code 2.1.304, 2.1.305 and 2.1.309 only. Claude Code as
installed on the development machine on 2026-10-29 is 2.1.312 (`claude --version`), so the default
harness, the path the getting-started guide now leads a newcomer through, stops before its first
step unless they install a pinned version and name it with `--harness-program`.

Seen while rewriting the public guides for 0.21.0 (wave 44 follow-up): a Claude Code run could
not be recorded for the console sample.

## Check

```expected_before
command: grep -A1 'name = "metaharness-claude"' Cargo.lock
exit: 0
healthy_exit: 0
output: version = "0.23.0"
source: Cargo.lock
```

The predicate holds while takt pins the metaharness release whose Claude adapter names no
2.1.31x version; it clears when the pin moves to a metaharness that qualifies the current Claude
Code. The fix is upstream (metaharness), then a loom and takt pin move.

## Outcome

The default harness runs with the Claude Code a newcomer installs today, or the install guide
names the version to install and the command that installs it.
