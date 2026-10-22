---
format: aep.planning-md/3
id: story:codex-step-rereads-its-context
kind: story
status: draft
title: A Codex step reads again the context files it was given
tags:
- finding
- rsi
refs:
- provider: finding
  reference: codex-step-rereads-its-context
relations:
- serves: vision:safe-agentic-coding
revision: 2
---
# A Codex step reads again the context files it was given

Since `story:codex-step-context`, a Codex judgement step is given `AGENTS.md` and the intent's
scope files as metaharness context files, inlined ahead of the prompt
(`crates/takt/src/harness.rs`, `context_files`). The prompt after them still carries the Codex
rule "read the files of this directory with shell commands" and says nothing of the files above
it, so the model reads them again.

What it cost: on Codex, scenario 1, five runs at `cabc09a2`, `tests.write` read an inlined file
again with a shell command in 3 of 5 runs, about 16k tokens for one more request; `takt
scorecard --against` the set without context files found `tests.write` not better.

The fix names the given files in the prompt, one line after the intent ("Files in scope, given
above in full: <paths>; do not read them again."), and the rule reads "read any other file of
this directory". A prompt with no context files is unchanged. The predicate below puts a fake
`codex` first on the `PATH` metaharness gives its child, records the arguments it is started
with, and looks for that line among them. The session ends without a rollout either way; only
what the Codex child was handed is judged.

```expected_before
scenario: 1
command: mkdir -p "$HOME/.local/bin" && printf '#!/bin/sh\nprintf "%%s\\n" "$@" >> "%s/codex-argv"\n' "$HOME" > "$HOME/.local/bin/codex" && chmod +x "$HOME/.local/bin/codex" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --harness codex --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 6 > "$HOME/run.log" 2>&1; test -s "$HOME/codex-argv" || { cat "$HOME/run.log"; echo "no codex child was started" >&2; exit 2; }; grep -q "given above in full" "$HOME/codex-argv" && exit 0; echo "a Codex step is not told its context files were given" >&2; exit 9
exit: 9
healthy_exit: 0
output: a Codex step is not told its context files were given
source: .engineering/planning/story/codex-step-context.md
```
