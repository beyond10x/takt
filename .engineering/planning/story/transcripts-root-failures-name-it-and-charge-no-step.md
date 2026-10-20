---
format: aep.planning-md/3
id: story:transcripts-root-failures-name-it-and-charge-no-step
kind: story
status: implemented
title: A transcripts root that cannot be made is named as such and charges no step
summary: a file at the transcripts path is called the set-aside directory; a partial failure charges the step
tags:
- finding
- rsi
refs:
- provider: finding
  reference: transcripts-root-failures-name-it-and-charge-no-step
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T00:48:40Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T00:48:40Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T01:41:22Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: transcripts-root-failures-name-it-and-charge-no-step

## Outcome

When the transcripts directory cannot be made, standard error names it as the transcripts
directory, and every directory takt made on the way before the failure is taken as takt's, so no
step is charged with it.

## Context

Found by the wave 17 confirm review at 00a900ac (docs/waves/wave-17.md), left for later. Wave 17
made the transcripts directory when the run opens (`Run::transcripts_in`,
`crates/takt/src/run.rs:6674` at 00a900ac). Two of its failure paths are wrong.

- The name. `transcripts_in` checks the path with `crate::git::set_aside_root`, whose errors say
  "the set-aside directory" (`crates/takt/src/git.rs:2107`). A transcripts path that is a file
  prints "no step transcript is kept: the transcripts directory <path>: the set-aside directory
  <path> cannot be made: <path> is not a directory" (`crates/takt-cli/src/main.rs:1378`). The
  set-aside directory is a different directory, and is fine.
- The parts made before a failure. `make_transcripts_root` (`crates/takt/src/harness.rs:471`)
  returns the directories it made only when every part succeeds; a part that fails returns the
  error through `?` (`harness.rs:482`), so `transcripts_in` never calls `Outside::found_made`
  (`run.rs:5258`) for the parts already made. When one of them is on the way to a watched
  directory (`~/.local` on the way to `~/.local/bin` on `PATH`), the first step is refused for
  it, which is the wave 17 F1 defect again, by a different route.

The predicate makes both happen. Run A: `~/.local/state/takt/transcripts` is a file; its standard
error is read for the set-aside wording. Run B: `~/.local/bin` is on `PATH` and `XDG_STATE_HOME`
is `~/.local/` followed by a 300-character name, so takt makes `~/.local` and then fails with
`ENAMETOOLONG`. Its first step, `tests.run`, is refused naming `~/.local`. Exit 9 says either
defect is there. Exit 0 says neither is.

## Acceptance

1. A transcripts path that cannot be made (a file on the way, a link, a name too long) is named
   on standard error as the transcripts directory, never as the set-aside directory: a test.
2. A failure part way through making the transcripts directory leaves every directory takt made
   before it taken as found when the run began (or removed again), so the run's first step is not
   refused for it: a test with a watched directory below the made part.
3. At the candidate, `scripts/check-finding
   story:transcripts-root-failures-name-it-and-charge-no-step <candidate>` prints
   `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/.local/state/takt"; : > "$HOME/.local/state/takt/transcripts"; "$TAKT" run --repo . --intent "Remove a todo by its number" --harness none --forge none --test-cmd ./check.sh --iterations 1 > "$HOME/a.out" 2>&1; sed "s|$HOME|~|g" "$HOME/a.out"; rm -rf "$HOME/.local"; sleep 1; printf '%s\n' '{"result":"{\"action\": \"implementation.edit\", \"summary\": \"an edit\"}","usage":{"input_tokens":1}}' > "$HOME/answer.json"; printf '%s\n' '#!/bin/sh' 'echo the work >> work.txt' "cat $HOME/answer.json" > "$HOME/bin/claude"; chmod +x "$HOME/bin/claude"; long=$(printf 'x%.0s' $(seq 300)); PATH="$HOME/bin:$HOME/.local/bin:$PATH" XDG_STATE_HOME="$HOME/.local/$long" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 2 > "$HOME/b.out" 2>&1; sed "s|$HOME|~|g; s|$long|<300 x>|g" "$HOME/b.out"; if grep -q 'the set-aside directory' "$HOME/a.out" || grep -q 'StepRefused' "$HOME/b.out"; then echo "the transcripts directory's failures misname it or charge a step with what takt made" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: the transcripts directory's failures misname it or charge a step
source: docs/waves/wave-17.md
```
