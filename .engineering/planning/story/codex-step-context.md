---
format: aep.planning-md/3
id: story:codex-step-context
kind: story
status: draft
title: A Codex judgement step is given no AGENTS.md and no scope files
tags:
- finding
- rsi
refs:
- provider: finding
  reference: codex-step-context
revision: 2
---
# A Codex judgement step is given no AGENTS.md and no scope files

A Claude Code step finds the repository's `AGENTS.md` and the story's scope files through the
`@<path>` mentions in its prompt. Codex expands no `@<path>` mention, so a Codex step got the
prompt and nothing else: no `AGENTS.md` and no scope file reached the session
(`crates/takt/src/harness.rs`, the session's `context` left empty for `Kind::Codex`).

What it cost: in 25 of 100 Codex runs of a trial at `0fab445d`, the model listed the tree and
read `AGENTS.md` itself, about 17k more tokens on `tests.write`. On Codex, scenario 1, five runs
a side, the median run took 101,585 tokens with the files handed to the session (`cabc09a`)
against 114,398 without (`8d15303`).

The fix hands the session `AGENTS.md` and the scope files as context files on metaharness
0.11.0, which inlines each ahead of the prompt (`cabc09a2`, `86d0433a`). The predicate below
puts a fake `codex` first on the `PATH` metaharness gives its child, records the arguments it
is started with, and looks for a line of the scenario's `AGENTS.md` among them. The session
ends without a rollout either way; only what the Codex child was handed is judged.

```expected_before
scenario: 1
command: mkdir -p "$HOME/.local/bin" && printf '#!/bin/sh\nprintf "%%s\\n" "$@" >> "%s/codex-argv"\n' "$HOME" > "$HOME/.local/bin/codex" && chmod +x "$HOME/.local/bin/codex" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --harness codex --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 6 > "$HOME/run.log" 2>&1; test -s "$HOME/codex-argv" || { cat "$HOME/run.log"; echo "no codex child was started" >&2; exit 2; }; grep -q "Python 3 standard library only" "$HOME/codex-argv" && exit 0; echo "a Codex step was given no AGENTS.md" >&2; exit 9
exit: 9
healthy_exit: 0
output: a Codex step was given no AGENTS.md
source: .engineering/planning/approval-record/trial-2026-10-21-bb4072a3-hand-over.md
```
