---
format: aep.planning-md/3
id: story:step-spends-a-request-reading-its-scope-files
kind: story
status: implemented
title: A judgement step spends a model request reading the files its story's scope names
tags:
- finding
- rsi
refs:
- provider: finding
  reference: step-spends-a-request-reading-its-scope-files
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T00:08:33Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-21T00:08:33Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
- {from: "active", to: "implemented", at: "2026-10-21T00:08:33Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# A judgement step spends a model request reading the files its story's scope names

## What is wrong

A judgement step's prompt names the files the change touches, in the story's `## Scope` section
inside the intent (`crates/takt/src/harness.rs:198-241`: the intent goes in whole), and gives
none of their contents. Claude Code's `Edit` refuses a file the session has not read. Each step is
a fresh `claude -p` with `--no-session-persistence` (`crates/takt/src/harness.rs:680-684`,
`720-738`), so every step opens with one request that only reads the scope's files, and its
results are paid again in every request after it.

The record: takt 0.6.0 (fd395d22) on scenario 1, 5 runs, 15 harness steps. The figures come from
each step's stream-json transcript (per-request `usage`, tool calls) and from `takt report`.

| figure | step 3 `tests.write` | step 6 `implementation.edit` | step 9 `change.handover` |
|---|---|---|---|
| first request's only tool calls, runs | `Read todo.py`, `Read tests/test_todo.py`: 5 of 5 | the same: 5 of 5 | the same: 5 of 5 |
| requests in the step | 3 in 5 runs | 3 in 3 runs, 4 in 2 | 2 in 5 runs |
| request 2's cache read (prefix and prompt, re-read for the round trip) | 6,273–6,276 (6,459 on the one cold call) | 6,338–6,352 | 6,380–6,408 |
| request 2's cache write (the two files as read) | 1,952–1,963 | 2,325–2,352 | 2,477–2,513 |
| `step_tokens` | 24,511–25,175 | 26,183–26,258 (3 requests), 35,874–35,984 (4) | 15,812–15,898 |

Run tokens: 66,506–76,434, median 67,331 (`takt scorecard`). Without that first request, a step
would carry the files in its first request and edit at once, so the request whose cache read is
the prefix and the prompt alone, about 6,300 tokens, goes.

Claude Code 2.1.304 attaches a file named `@<path>` in the prompt before the first request and
reads it through the Read tool's path, recording it as read (read from the 2.1.304 bundle,
`at_mentioned_files` and `readThroughMention`; inferred, not run in print mode).

Estimate (inferred, not run), a range the re-trial decides:

- Best case, at fd395d22: steps 3, 6 and 9 each lose one request, about 6,270 + 6,340 + 6,390
  ≈ 19,000 tokens a run, a median near 48,300. With the change so far in the prompt (wave 23),
  step 9 already hands over in one request and would pay the files once (about +2,500): net
  about -10,100 a run.
- Low: only `tests.write` loses its read request, about -6,000 a run.
- Worst case: print mode attaches the files and `Edit` still wants a `Read`, so the files are paid
  twice in each of the run's 8 requests, about +2,000 each, +16,000 a run.

## Change

`ClaudeCode::prompt` appends one line after the intent naming each path the intent's `## Scope`
section gives in backticks that is a regular file inside the tree (relative, no `..`), as a
Claude Code file mention: `Files in scope, attached: @todo.py @tests/test_todo.py`. At most 8
files and 64 KiB in total; a path past the bound, missing or not a file is left out. No other
harness and no other part of the prompt changes.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/fake" && printf '%s\n' 'Remove a todo by its number' '' '## Scope' '' '- `todo.py` (a `remove` command beside `add` and `done`)' '- `tests/test_todo.py` (the tests)' > "$HOME/fake/intent.md" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a marker line\"}"}' > "$HOME/fake/tests-write.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"repository.merge\", \"summary\": \"the scope files are attached\"}"}' > "$HOME/fake/approval.json" && printf '%s\n' '#!/bin/sh' 'f="$HOME/fake"' 'in=$(cat)' 'if [ -e "$f/called" ]; then cat "$f/approval.json"; exit 0; fi' 'case "$*" in *@todo.py*|*@./todo.py*) ;; *) echo "{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{\\\"action\\\": \\\"prompt-attaches-no-scope-file\\\", \\\"summary\\\": \\\"no scope file is attached\\\"}\"}"; exit 0 ;; esac' 'case "$*" in *@tests/test_todo.py*|*@./tests/test_todo.py*) ;; *) echo "{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{\\\"action\\\": \\\"prompt-attaches-no-scope-file\\\", \\\"summary\\\": \\\"no scope file is attached\\\"}\"}"; exit 0 ;; esac' 'touch "$f/called"' 'echo "# a marker line" >> tests/test_todo.py' 'cat "$f/tests-write.json"' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "$(cat "$HOME/fake/intent.md")" --forge none --test-cmd ./check.sh --iterations 6
exit: 3
healthy_exit: 4
output: StepRefused (prompt-attaches-no-scope-file was not offered)
source: crates/takt/src/harness.rs
```

The run's intent carries a `## Scope` section naming `todo.py` and `tests/test_todo.py`, as the
scenario's story does. The fake `claude` reads its arguments at its first call. A prompt that
mentions neither file as `@<path>`: it exits 1, so the run stops on the harness error, exit 5.
A prompt that mentions both: it performs `tests.write` (a comment line appended to the test
file), and at its second call it answers `repository.merge`, an action the second step offers
under its approvals, so the run stops `ApprovalRequired`, exit 4. Any other error of a candidate
stays exit 5 without the defect's text. Checked at fd395d22 outside the confinement: exit 5,
`claude exited 1: prompt-attaches-no-scope-file` at step 2; the same command with a fake that
skips the mention check gave exit 4, `stopped: ApprovalRequired (repository.merge); steps 4`.

## Scope

- `crates/takt/src/harness.rs`: `ClaudeCode::prompt` and a helper that reads the scope's paths
  from the intent and keeps the files of the tree; tests beside the prompt's own. The same file as
  the change so far (wave 23); the two blocks are independent.
- `ess/domains/takt.yaml` and `ess/domains/takt.citations.tsv`, only if a cited line of
  `harness.rs` moves.

## Acceptance

- `scripts/check-finding` gives `not_reproduced` at the candidate's sha (the predicate exits 4).
- Five scenario-1 trials on `--harness claude` at the candidate keep the outcome (5 of 5 `ApprovalRequired`, held-out acceptance passing) and, against five at the revision it was cut from, `takt scorecard --against … --steps tests.write` is better and the run is not worse.
- Five scenario-1 trials on `--harness codex` at the candidate are not worse than five at that revision: the mention line is on the Claude route only.
- The requests of each tests.write and implementation.edit step are recorded, and no candidate step's first request only reads scope files.
