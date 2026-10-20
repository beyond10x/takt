---
format: aep.planning-md/3
id: story:transcripts-under-a-python-dir-charge-no-step
kind: story
status: implemented
title: Transcripts under a directory Python imports from charge no step
summary: with PYTHONUSERBASE at ~/.local the first step is refused for the transcripts directory
tags:
- finding
- rsi
refs:
- provider: finding
  reference: transcripts-under-a-python-dir-charge-no-step
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T00:48:40Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T00:48:40Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T01:41:23Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: transcripts-under-a-python-dir-charge-no-step

## Outcome

A run whose `PYTHONUSERBASE`, or a `PYTHONPATH` entry, names `~/.local` itself (or any directory
that holds the transcripts directory) runs as one without it: no step is refused for the
transcripts directory takt made when the run opened, nor for a transcript takt writes after a
step.

## Context

Found by the wave 17 confirm review at 00a900ac (docs/waves/wave-17.md), left for later. The step
guard holds everything under each directory Python imports from outside the tree
(`outside_import_dirs`, `crates/takt/src/run.rs:1690` at 00a900ac): every `PYTHONPATH` entry and
the value of each of `PYTHON_DIRS` (`run.rs:2164`), `PYTHONUSERBASE` among them, each walked whole
(`under`, `run.rs:5195`). The default transcripts directory is
`$XDG_STATE_HOME/takt/transcripts`, `~/.local/state/takt/transcripts` without it
(`default_transcripts`, `run.rs:6131`). So with `PYTHONUSERBASE=~/.local`, which is Python's own
default made explicit, the transcripts directory is inside a guarded import directory.

`transcripts_in` (`run.rs:6674`) makes the directory at open and hands what it made to
`Outside::found_made` (`run.rs:5258`), which updates only paths the first snapshot already holds.
On an empty home that is `~/.local`. `~/.local/state`, `~/.local/state/takt` and
`~/.local/state/takt/transcripts` were not there when the snapshot was taken, so it holds none of
them, and the first step is refused for all three. Once that is fixed, each transcript written
after a harness step (`keep_step_transcript`, `crates/takt/src/harness.rs`) is a new path under
the same import directory and would be charged to the next step.

The predicate runs takt with `PYTHONUSERBASE="$HOME/.local"` on an empty home and a `claude`
double that edits. At 00a900ac step 1 (`tests.run`) is refused, naming `~/.local/state`. Exit 9
says a step was refused for something under `~/.local/state`. Exit 0 says none was.

## Acceptance

1. With `PYTHONUSERBASE` set to `~/.local` and an empty home, a run takes its steps and keeps
   each transcript; no step is refused for the transcripts directory or a transcript: a test.
2. The same with a `PYTHONPATH` entry that is `~/.local`, and with `XDG_STATE_HOME` inside a
   guarded import directory: a test.
3. A step that writes a module into that import directory (outside the transcripts directory) is
   still refused: a test.
4. At the candidate, `scripts/check-finding story:transcripts-under-a-python-dir-charge-no-step
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin"; printf '%s\n' '{"result":"{\"action\": \"implementation.edit\", \"summary\": \"an edit\"}","usage":{"input_tokens":1}}' > "$HOME/answer.json"; printf '%s\n' '#!/bin/sh' 'echo the work >> work.txt' "cat $HOME/answer.json" > "$HOME/bin/claude"; chmod +x "$HOME/bin/claude"; PATH="$HOME/bin:$PATH" PYTHONUSERBASE="$HOME/.local" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 4 > "$HOME/run.out" 2>&1; sed "s|$HOME|~|g" "$HOME/run.out"; if grep -q "differs from what the run found when it began: $HOME/.local/state" "$HOME/run.out"; then echo "a step was charged with the transcripts directory takt wrote under PYTHONUSERBASE" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: a step was charged with the transcripts directory takt wrote
source: docs/waves/wave-17.md
```
