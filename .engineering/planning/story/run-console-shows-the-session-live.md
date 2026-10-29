---
format: aep.planning-md/3
id: story:run-console-shows-the-session-live
kind: story
status: draft
title: A takt run's console shows the harness session live
summary: read/edit/run/says/thinking/tokens sub-lines from the session's events as they arrive; needs a Loom session-event observer
tags:
- finding
- rsi
refs:
- provider: finding
  reference: run-console-shows-the-session-live
relations:
- serves: vision:safe-agentic-coding
- depends_on: story:run-console-steps-are-compact
scope:
- confidence: cited
  path: Cargo.toml
- confidence: inferred
  path: crates/takt/src/console.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/tests/run_console_live.rs
revision: 4
---
# A takt run's console shows the harness session live

While a harness session runs, a developer watching `takt run` sees nothing of what it does: the
tool calls, the files it reads and edits, the commands it runs and their exit status, its text,
its thinking and its tokens reach takt only when the session has ended, in the transcript and the
step record.

Tagged `rsi`: found in the owner's watch of a self-run of takt on one of its own stories
(2026-10-28), a self-improvement trial; see story:run-console-opens-with-the-story.

Blocked by dependency-blocker:loom-session-event-observer: Loom's session executor hands takt no
event while the session runs.

The predicate drives a scripted `claude` named by `--harness-program` (no model is called) that
streams one `Read` tool call of `a.txt`, then replies `implementation.edit`. The defect is there
when no line before the step's result line names the file read. Run by hand on 2026-10-28 with
the installed takt 0.17.0 as `$TAKT`, in a one-file repository holding `a.txt` and a fresh HOME:
exit 9. `scripts/check-finding` at the promoted revision: not run; on scenario 1 the command needs
`a.txt` replaced by a file of that seed.

```expected_before
scenario: 1
command: h=$HOME; printf '%s\n' '{"type":"system","subtype":"init","model":"fake-model","claude_code_version":"2.1.309","tools":["Read","Edit","Write","Glob","Grep"]}' '{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"a.txt"}}]}}' '{"type":"result","subtype":"success","result":"{\"action\": \"implementation.edit\", \"summary\": \"read a.txt\"}","usage":{"input_tokens":3,"output_tokens":4}}' > "$h/s.json"; printf '%s\n' '#!/bin/sh' 'if [ "$1" = --version ]; then echo "2.1.309 (Claude Code)"; exit 0; fi' "cat $h/s.json" > "$h/claude"; chmod +x "$h/claude"; git config user.name t && git config user.email t@example.invalid && "$TAKT" run --repo . --intent "Read a.txt" --harness claude --harness-program "$h/claude" --forge none --test-cmd true --min-free-gib 0 --iterations 2 > "$h/run.log" 2>&1; grep -q '^step 2: implementation.edit (' "$h/run.log" || { cat "$h/run.log" >&2; echo "the run took no harness step 2" >&2; exit 2; }; sed -n '/^step 2: implementation.edit (/q;p' "$h/run.log" | grep -q '^ *read a.txt' && exit 0; echo "no live line named the file the session read" >&2; exit 9
exit: 9
healthy_exit: 0
output: no live line named the file the session read
source: crates/takt/src/harness.rs
```

## Outcome

Between a harness step's working line and its result line, `takt run` prints one indented
sub-line per session event as it happens: `read <path>`, `edit <path>`, `run <command> (exit N)`,
`says <text>`, `thinking ~N`, `tokens N`, throttled so a burst of events stays readable, every
path relative to the step's copy of the tree.

## Context

- Loom 0.46.0, which takt pins (`Cargo.toml:16`), polls the metaharness session in one loop
  (`crates/loom-session-executor/src/lib.rs:967-986` in Loom at tag 0.46.0, `run.poll_event()` at
  `:974`) and keeps each event in the report's transcript; it calls nothing of the caller's per
  event, and the report reaches takt when the session has ended.
- takt's `StepContext` (`crates/takt/src/harness.rs:19`) carries no channel for live output.
- The sub-lines need the metaharness events `ToolRequested`, `ToolResult`, `Text`,
  `ThinkingEstimate` and `Usage` as they arrive.
- After story:run-console-steps-are-compact, which prints the working line these follow.

## Acceptance

Observed by `crates/takt/tests/run_console_live.rs`:

1. A scripted session that reads a file, edits one and runs a command prints `read <path>`,
   `edit <path>` and `run <command> (exit N)` sub-lines, in that order, before the step's result
   line; the predicate above gives `not_reproduced`.
2. A burst of events past the throttle prints a bounded number of sub-lines and counts the rest.
3. No path in a sub-line is absolute.
4. `events.jsonl` and the step's transcript are unchanged.

## Scope

- `Cargo.toml`: the Loom pin, to the release that clears the blocker (cited)
- `crates/takt/src/harness.rs`: `StepContext` (`:19`) gains the live sink; the session
  executor's observer is set where the harness builds it (`:878`) (cited)
- `crates/takt/src/console.rs`: the sub-line shapes (inferred)
- `crates/takt/tests/run_console_live.rs`: new (inferred)
