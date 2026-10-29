---
format: aep.planning-md/3
id: story:run-console-steps-are-compact
kind: story
status: implemented
title: A takt run's step lines are compact and say what each step cost
summary: 'a working line before each harness call; time, tokens and cost on step lines; paths on effect: committed; wall time and commits on stopped:'
tags:
- finding
- rsi
refs:
- provider: finding
  reference: run-console-steps-are-compact
relations:
- serves: vision:safe-agentic-coding
- depends_on: story:run-console-opens-with-the-story
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/run_console_steps.rs
- confidence: inferred
  path: crates/takt/src/console.rs
- confidence: cited
  path: crates/takt/src/run.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T19:21:09Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-28T20:20:28Z", actor: "human:timo", revision: 6, executor: "agent:developer"}
- {from: "active", to: "implemented", at: "2026-10-29T21:30:17Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
# A takt run's step lines are compact and say what each step cost

While a harness works, `takt run` prints nothing: the step's line and its `said:` line come only
after the session returns (`crates/takt/src/run.rs:12633-12640`), so a developer watching cannot
tell a working step from a hung one. The step lines carry neither the step's time, its tokens nor
its cost, though `events.jsonl` records each (`Event::Step`, `crates/takt/src/events.rs:161`:
`tokens`, `cost_usd`, `duration_ms`); `effect: committed N path(s)` names no path
(`crates/takt/src/run.rs:13229`), though `Event::Effect` holds them (`:193`); and the `stopped:`
line gives steps and tokens only (`crates/takt-cli/src/main.rs:3232`).

Tagged `rsi`: found in the owner's watch of a self-run of takt on one of its own stories
(2026-10-28), a self-improvement trial; see story:run-console-opens-with-the-story.

Run by hand on 2026-10-28 with the installed takt 0.17.0 as `$TAKT`, in a one-file repository and
a fresh HOME: exit 9. `scripts/check-finding` at the promoted revision: not run.

```expected_before
scenario: 1
command: o="$HOME/run.log"; git config user.name t && git config user.email t@example.invalid && "$TAKT" run --repo . --intent "Remove a todo by its number" --harness none --forge none --test-cmd true --min-free-gib 0 --iterations 2 > "$o" 2>&1; grep -q '^step 2: repository.merge (none)' "$o" || { cat "$o" >&2; echo "the run took no harness step 2" >&2; exit 2; }; sed -n '/^step 2: repository.merge (none)/q;p' "$o" | grep -q '^step 2: .*working' && exit 0; echo "no working line before the harness was called" >&2; exit 9
exit: 9
healthy_exit: 0
output: no working line before the harness was called
source: crates/takt/src/run.rs
```

## Outcome

Before each harness call `takt run` prints `step N: <harness> working (may: <offered actions>)`,
so the step is visible while it runs. A harness step's line, after the call, appends the call's
duration, tokens and cost when the harness reported them; a validator step's line appends its
duration and its result in a few words (pass or fail, with the failed checks named). `effect:
committed N path(s); implementation is now <rev>` appends the paths it committed. The `stopped:`
line names, before its `, case <dir>`, the run's wall time, its total cost when any was reported,
and the commits and paths it made.

## Context

- The harness is called at `crates/takt/src/run.rs:12268` (`self.harness.step(&context)`), the
  `none` harness too; the step line and `said:` are written at `:12633-12640`, after it returns.
- Validator steps print `step N: <action> {}`, then `effect:`, `artifact:` and `evidence:`
  (`crates/takt/src/run.rs:11382-11427`).
- Effects: `:12931`, `:12951`, `:13078`, `:13176` and the committed effect at `:13229`.
- End lines: `candidate:` (`crates/takt/src/run.rs:10500`), `obligations:` (`:10539`), `stopped:`
  (`crates/takt-cli/src/main.rs:3221`, `:3232`).
- Each fact is recorded in `events.jsonl` before it is printed (`crates/takt/src/run.rs:11727`,
  `Events::emit` at `crates/takt/src/events.rs:333`), so the figures this story prints are read
  from what the step event already holds.
- Constraints from the tests: about 230 assertions read `step N: <action>`, `  said:`,
  `  effect:` and `stopped:` as prefixes, so each is only appended to. The `stopped:` line stays
  the last line (`crates/takt-cli/tests/protocol_defaults_cli.rs:570`), and the case directory
  stays at its end, after the last `, case ` (`crates/takt-cli/tests/decline_cli.rs:255-258`,
  `decline_adv1.rs:262`, `decline_sec1.rs:298-301`). The working line also starts `step N:`; a
  test that takes the first `step N:` line must still find the result line.
- Depends on story:run-console-opens-with-the-story: both change the same printing code in
  `crates/takt/src/run.rs` and the new `crates/takt/src/console.rs`.

## Acceptance

Observed by `crates/takt-cli/tests/run_console_steps.rs`:

1. Before each harness call, the `none` harness's included, the output holds
   `step N: <harness> working (may: …)` before that step's result line; the predicate above gives
   `not_reproduced`.
2. A harness step whose call reported tokens, cost and duration appends each to its
   `step N: <action> (<harness>)` line; one that reported none appends nothing for them.
3. `effect: committed N path(s); implementation is now <rev>` is followed on that line by the N
   committed paths, relative to the tree.
4. The `stopped:` line keeps `stopped: <stop>; steps N, tokens N`, ends with `, case <dir>`, is
   the last line, and holds the run's wall time and its commit count between the two.
5. `events.jsonl` is unchanged.

## Scope

- `crates/takt/src/run.rs`: the harness call at `:12268`, the step lines at `:12633-12640`, the
  validator step at `:11382-11427`, the committed effect at `:13229` (cited)
- `crates/takt/src/console.rs`: the line shapes, from story:run-console-opens-with-the-story
  (inferred)
- `crates/takt-cli/src/main.rs`: the `stopped:` lines at `:3221`, `:3232` (cited)
- `crates/takt-cli/tests/run_console_steps.rs`: new (inferred)
