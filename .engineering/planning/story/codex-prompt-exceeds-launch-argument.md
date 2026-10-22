---
format: aep.planning-md/3
id: story:codex-prompt-exceeds-launch-argument
kind: story
status: draft
title: A Codex step's prompt and context files can exceed one launch argument
tags:
- finding
- rsi
refs:
- provider: finding
  reference: codex-prompt-exceeds-launch-argument
revision: 2
---
# A Codex step's prompt and context files can pass one launch argument

A Codex judgement step is given `AGENTS.md` and the intent's scope files as metaharness context
files, which metaharness inlines ahead of the prompt into the one argument the `codex` child is
started with. takt bounds those files by `SCOPE_BYTES`, 64 KiB, and charges each its bytes and
its path (`crates/takt/src/harness.rs:749-758`), but nothing counts the prompt they are put in
front of. metaharness 0.11.0 refuses a launch whose argument is 131,072 bytes or more
(`ARGUMENT_BYTES_MAX`, `crates/metaharness-protocol/src/spec.rs:674`; checked against the
composed prompt by `guard_argument_lengths`, `crates/metaharness-codex/src/launch.rs:1045-1066`,
called at `:739`), and its 64 KiB cap on context files leaves the task to the adapter
(`spec.rs:661`).

The effect: an intent of more than about 62 KiB whose scope files fill the budget is refused
before the Codex child starts ("metaharness refused the session: ... the prompt would be N bytes,
and Linux refuses one argument of 131072 bytes or more"), and the step fails, where the same
intent started a session before the context files were handed over (`86d0433a`). The prompt
alone stays well under the limit (`CHANGE_SHOWN_BYTES` bounds the change it shows); the context
files are what push it past.

The predicate below commits seven filler files to the scenario's checkout, names them in the
intent's `## Scope` with about 72 KiB of further intent text, and runs one Codex step with a fake
`codex` first on the `PATH` metaharness gives its child, which records its arguments. The defect
is there when no `codex` child was started and takt's output names metaharness's refusal; it is
gone when the child was started.

```expected_before
scenario: 1
command: mkdir -p "$HOME/.local/bin" && printf '#!/bin/sh\nprintf "%%s\\n" "$@" >> "%s/codex-argv"\n' "$HOME" > "$HOME/.local/bin/codex" && chmod +x "$HOME/.local/bin/codex" && git config user.name t && git config user.email t@example.invalid && mkdir -p notes && for n in 32000 16000 8000 4000 2000 1000 500; do yes 'a line of notes that fills the context budget' | head -c "$n" > "notes/pad-$n.txt"; done && git add notes && git -c user.name=t -c user.email=t@example.invalid commit -q -m 'notes in scope' && scope=$(for n in 32000 16000 8000 4000 2000 1000 500; do printf '`notes/pad-%s.txt` ' "$n"; done) && notes=$(yes 'a sentence of the intent that makes it long' | head -c 73728) && XDG_STATE_HOME="$HOME/state" "$TAKT" run --repo . --intent "$(printf 'Remove a todo by its number\n\n## Scope\n\n%s\n\n## Notes\n\n%s\n' "$scope" "$notes")" --harness codex --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 6 > "$HOME/run.log" 2>&1; test -s "$HOME/codex-argv" && exit 0; grep -q "Linux refuses one argument of 131072 bytes" "$HOME/run.log" && { echo "a Codex step was refused: its prompt and context files pass one launch argument" >&2; exit 9; }; cat "$HOME/run.log"; echo "no codex child was started, and no launch refusal was named" >&2; exit 2
exit: 9
healthy_exit: 0
output: its prompt and context files pass one launch argument
source: .engineering/planning/story/codex-step-context.md
```
