---
format: aep.planning-md/3
id: story:a-turn-does-not-open-when-the-mainline-tip-cannot-be-read
kind: story
status: implemented
title: A turn does not open when the mainline tip cannot be read
summary: the merged-candidate refusal fails open when the mainline tip cannot be read
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-turn-does-not-open-when-the-mainline-tip-cannot-be-read
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T00:48:39Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T00:48:40Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T01:41:22Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a-turn-does-not-open-when-the-mainline-tip-cannot-be-read

## Outcome

An improvement turn opens only when takt has read the mainline's tip and found that it does not
hold the candidate. If the tip cannot be read (the bot route is down, the network is gone, the
forge has no `main`), the opening is refused before anything is written. The refusal names the
forge, why the read failed, and what to do: open again once the tip can be read.

## Context

Found by the wave 17 adversarial review at ef8609da (docs/waves/wave-17.md, unit B). Wave 17 made
`turn_opening` refuse a candidate the mainline already holds, because such a candidate was merged
before a turn could grant it (`crates/takt/src/case.rs:2608-2621` at 00a900ac). The refusal
needs the tip. `Mainline::tip` (`crates/takt/src/improvement.rs:1169-1194`) reads it through the
forge the run names, or from the local `main` with `--forge none`. When that read fails, the
code says that "a tip that cannot be read refuses nothing here" (`case.rs:2608-2610`; doc
comment `case.rs:2533-2535`), and the opening goes on. So a candidate that `main` already holds
opens a turn whenever the route is down. That is the case wave 17 meant to close, and the check
fails open exactly when it cannot see.

The predicate makes a candidate commit and points the local `main` at it. With `--forge none` the
opening is refused as merged, which shows the setup holds (exit 2 otherwise). Then `origin` is
pointed at a repository the bot route serves, the route's programs are replaced by doubles that
fail, and the turn is opened with `--forge bot`. The opening gets past the merged-candidate
check and stops only at the next precondition, the empty promotion store. Exit 9 says the
opening went on with no tip read. Exit 0 says it stopped before that, as a fix refusing an
unreadable tip would.

## Acceptance

1. `turn_opening` refuses when `Mainline::tip` fails, naming the forge and the read's error, and
   writes nothing. A test with a forge double whose tip read fails.
2. A turn that resumes is unchanged: only an opening reads the tip this way. A test.
3. The help and the `takt.work.Run` inputs comment list the new refusal.
4. At the candidate, `scripts/check-finding
   story:a-turn-does-not-open-when-the-mainline-tip-cannot-be-read <candidate>` prints
   `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: set -u; g="git -c user.name=probe -c user.email=probe@example.invalid"; c=$($g commit-tree -p HEAD -m candidate "HEAD^{tree}"); git branch -f main "$c"; mkdir -p "$HOME/bin" "$HOME/state"; printf '%s\n' '#!/bin/sh' 'echo "the route is down" >&2' 'exit 1' > "$HOME/bin/b10x-gates"; cp "$HOME/bin/b10x-gates" "$HOME/bin/gh"; chmod +x "$HOME/bin/b10x-gates" "$HOME/bin/gh"; export PATH="$HOME/bin:$PATH" XDG_STATE_HOME="$HOME/state"; "$TAKT" run --repo . --protocol self.improvement/1 --input finding=story:remove-a-todo --input candidate="$c" --harness none --forge none --iterations 0 > "$HOME/none.out" 2>&1; echo "--forge none: exit $?"; sed "s|$HOME|~|g; s|$c|<candidate>|g" "$HOME/none.out"; grep -q 'so it was merged before a turn could grant it' "$HOME/none.out" || { echo "setup: the local main does not hold the candidate" >&2; exit 2; }; git remote add origin https://github.com/example/todo-app.git; git update-ref refs/remotes/origin/main "$c"; "$TAKT" run --repo . --protocol self.improvement/1 --input finding=story:remove-a-todo --input candidate="$c" --harness none --forge bot --iterations 0 > "$HOME/bot.out" 2>&1; echo "--forge bot, route down: exit $?"; sed "s|$HOME|~|g; s|$c|<candidate>|g" "$HOME/bot.out"; if grep -q 'holds no baseline or promotion record' "$HOME/bot.out"; then echo "the mainline tip could not be read and the turn's opening went on past the merged-candidate refusal" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: the turn's opening went on past the merged-candidate refusal
source: docs/waves/wave-17.md
```
