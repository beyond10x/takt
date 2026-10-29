---
format: aep.planning-md/3
id: story:console-shows-reads-and-diffs
kind: story
status: active
title: The console shows what a step read and the diff it made, in colour
summary: After each harness step the console lists the step's tool calls and the diff of its commit, escaped and bounded, with colour under --color auto|always|never
tags:
- owner-request
- priority:high
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/console.rs
- confidence: cited
  path: crates/takt/src/trace.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T08:33:52Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T08:33:52Z", actor: "human:timo", revision: 3}
---
# The console shows what a step read and the diff it made, in colour

## Why

A person watching `takt run` sees one line per step: `step 8: implementation.edit (codex) ·
36.8s`, what the harness said, and `effect: committed 2 path(s); implementation is now e650c2b6:
<paths>`. What the agent looked at, and what it changed, are only in the step's transcript and in
git. To follow a run (and to stop one that has gone wrong) the person has to open both by hand.
The owner asked for this on 2026-10-29 while watching wave 44's runs.

## Outcome

After each harness step, under its `said:` line and before its `effect:` line, the console prints:

1. **What the step looked at.** One line per tool call in the step's transcript, in order: for a
   shell command (Codex's `exec_command`, Claude's `Bash`) the command line; for a file tool
   (Claude's `Read`, `Grep`, `Glob`, `Edit`, `Write`) the tool and the path or pattern. Each line
   is escaped as every other console line is (`crate::trace::escape`: no control or bidi
   character, no terminal escape from the transcript reaches the terminal), cut at the console's
   width with ` …`, and at most 20 lines per step, then `… <n> more`.
2. **What the step changed.** When the step committed, the unified diff of that commit
   (`git diff <before>..<after>`, outside `.engineering/` and `.takt/`), escaped the same way, at
   most 120 lines per step, then `… <n> more lines (git show <sha>)`. A step that committed
   nothing prints no diff.
3. **Colour.** With colour on: the step line's action and the pass/fail result (green pass, red
   fail), `said:` and `effect:` labels dim, diff lines added green, removed red, hunk headers
   cyan, tool-call lines dim. `takt run --color auto|always|never`, default `auto`: on when
   standard output is a terminal and `NO_COLOR` is unset or empty, as <https://no-color.org>
   says. `always` is for a run whose output is piped through `tee` into a terminal. Colour is
   never written to the case's records (`events.jsonl`, evidence); only the console carries it.

`takt run --quiet-steps` (or the run file's `console: {reads: false, diff: false}`) turns the two
listings off; nothing else changes. The record a run writes is unchanged by this story.

## Out of scope

- Streaming tool calls while the harness step is still running. The listing comes when the step
  ends; a live view needs the session executor to hand takt its events as they arrive, a later
  story.
- Any change to what a transcript keeps, or to `takt trace` / `takt report`.

## Acceptance

1. A run's console, at a step whose transcript holds two shell commands and whose commit changes
   one file, prints both commands and that file's diff between the step's `said:` and `effect:`
   lines (a test over the console functions with a fixture transcript of each harness, Codex and
   Claude).
2. A transcript command holding `\x1b[2J` or a U+202E prints them escaped (test).
3. `--color never` and `NO_COLOR=1` with `auto` print no escape sequence at all; `--color always`
   prints a diff's added line with the green sequence (tests).
4. A diff over 120 lines ends with the `… <n> more lines (git show <sha>)` line (test).
5. `task check` passes; the ESS prose that describes the console (`ess/domains/takt.yaml`) says
   what this story adds, with citations.

## Decision (wave 44, review)

The adversarial pass on 75fc3867 found five cases red, in
`crates/takt/tests/w44_adv_console_listing.rs`. The planner's decision:

- A diff line is a file header (`--- `, `+++ `) only between its `diff --git` line and the first
  `@@`; inside a hunk a line starting `-` is removed (red) and `+` added (green), whatever
  follows.
- Escaping covers every Bidi_Control character, U+061C, U+200E and U+200F among them, in every
  console line (the `said:` line too).
- A line is cut at 100 terminal columns, a wide character (CJK, emoji) counting two.
- A tab in a listed command or diff line prints as spaces to the next multiple of 4 columns, not
  as `\u{9}`; every other control character stays escaped.
- Every tool call is one line: a Codex `custom_tool_call` whose input holds no `tools.` call
  prints its name; an apostrophe inside a `//` or `/* */` comment of a Codex script opens no
  string.
