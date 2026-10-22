---
format: aep.planning-md/3
id: story:codex-context-not-recorded
kind: story
status: active
title: A Codex step's record does not name the context files it was given
tags:
- finding
- rsi
refs:
- provider: finding
  reference: codex-context-not-recorded
relations:
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T00:17:24Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T00:17:24Z", actor: "human:timo", revision: 4}
---
# A Codex step's record does not name the context files it was given

A Codex judgement step is given `AGENTS.md` and the intent's scope files as metaharness context
files (`context_files`, `crates/takt/src/harness.rs:437-441`). The step's opening record,
`takt.prompt`, holds the step and the prompt and nothing else (`crates/takt/src/harness.rs:417-422`),
and it is the record the step's transcript starts with
(`$XDG_STATE_HOME/takt/transcripts/<repo>/<case>/run-<n>/step-<n>.jsonl`, `keep_transcript`,
`crates/takt/src/harness.rs:1278-1290`). Which files were handed over is decided at that step from
the tree, git's index and the change so far, and written nowhere.

The effect: neither the case's files nor the step's transcript show what a Codex step was given
besides its prompt, so a trace cannot attribute a step's tokens to its context and a replay of the
step cannot hand the session the same files: two steps with one prompt read the same in the record
and may have been given different files.

The predicate below runs one Codex step with a fake `codex` first on the `PATH` metaharness gives
its child (as in `finding:codex-step-context`), then reads the `takt.prompt` records the run kept.
The prompt text of a Codex step names no `AGENTS.md` (Codex is given no `@<path>` mention), so the
defect is there when no `takt.prompt` record names `AGENTS.md`, and gone when one does.

```expected_before
scenario: 1
command: mkdir -p "$HOME/.local/bin" && printf '#!/bin/sh\nprintf "%%s\\n" "$@" >> "%s/codex-argv"\n' "$HOME" > "$HOME/.local/bin/codex" && chmod +x "$HOME/.local/bin/codex" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --harness codex --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 6 > "$HOME/run.log" 2>&1; test -s "$HOME/codex-argv" || { cat "$HOME/run.log"; echo "no codex child was started" >&2; exit 2; }; grep -rhs '"takt.prompt"' "$HOME/state/takt/transcripts" .engineering/cases > "$HOME/prompts"; test -s "$HOME/prompts" || { cat "$HOME/run.log"; echo "no takt.prompt record was kept" >&2; exit 2; }; grep -q "AGENTS.md" "$HOME/prompts" && exit 0; echo "a Codex step's prompt record names no context file" >&2; exit 9
exit: 9
healthy_exit: 0
output: a Codex step's prompt record names no context file
source: .engineering/planning/story/codex-step-context.md
```
