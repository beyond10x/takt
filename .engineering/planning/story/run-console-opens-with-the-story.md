---
format: aep.planning-md/3
id: story:run-console-opens-with-the-story
kind: story
status: active
title: A takt run's console opens with the story and keeps the frontier quiet
summary: 'header with story id, title and Outcome summary; may: / may now: instead of a frontier line per decision; full frontier under --verbose'
tags:
- finding
- rsi
refs:
- provider: finding
  reference: run-console-opens-with-the-story
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/run_console_cli.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/console.rs
- confidence: cited
  path: crates/takt/src/run.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T19:21:08Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-28T19:21:09Z", actor: "human:timo", revision: 5}
---
# A takt run's console opens with the story and keeps the frontier quiet

A developer watching `takt run` cannot tell at a glance which story runs, and each decision prints
the whole frontier, every action with its status, blocked and approval-required ones included,
before every step (`crates/takt/src/run.rs:11743`, the line built by `Decision::frontier_line`,
`crates/takt/src/case.rs:2721`). In the owner's watch of a self-run of takt on one of its own
stories (2026-10-28) these lines were the bulk of the output and said least.

Tagged `rsi`: the owner's run of takt on its own story is a self-improvement trial (a run of the
promoted takt on takt's own work, watched for what to improve), and this story is what that watch
found.

Run by hand on 2026-10-28 with the installed takt 0.17.0 as `$TAKT`, in a one-file repository and
a fresh HOME: exit 9. `scripts/check-finding` at the promoted revision: not run.

```expected_before
scenario: 1
command: o="$HOME/run.log"; git config user.name t && git config user.email t@example.invalid && "$TAKT" run --repo . --intent "Remove a todo by its number" --harness none --forge none --test-cmd true --min-free-gib 0 --iterations 2 > "$o" 2>&1; grep -q '^step 1: tests.run' "$o" || { cat "$o" >&2; echo "the run took no first step" >&2; exit 2; }; grep -q '^frontier: .*(admissible)' "$o" || exit 0; echo "a decision printed its whole frontier without --verbose" >&2; exit 9
exit: 9
healthy_exit: 0
output: a decision printed its whole frontier without --verbose
source: crates/takt/src/run.rs
```

## Outcome

`takt run` opens with a header that names what runs: the takt version and the story's id (or the
intent's first line for `--intent`), the title, and a summary of one to three wrapped lines taken
from the intent's `## Outcome` section, or its first paragraph when it has none. The header ends
with `may: <actions>`, the admissible actions of the first decision. After that the frontier
prints only when the admissible set changes, as `may now: +<added> -<removed>`. The full
`frontier: <action> (<status>), …` line prints only under a new `--verbose` flag of `takt run`.
On a terminal the header may be coloured; to a file or a pipe every line is plain text.

## Context

- Where the lines come from today, at 338082d8: story selection, `frontier: took`
  (`crates/takt/src/run.rs:9904`, `crates/takt-cli/src/main.rs:3909`) and `frontier: resumed`
  (`crates/takt/src/run.rs:9940`, `crates/takt-cli/src/main.rs:1961`); `settings:`
  (`crates/takt-cli/src/main.rs:2055`, text at `crates/takt/src/protocol.rs:496`); the forge and
  scan lines (`crates/takt-cli/src/main.rs:3173`, `:3176`); `case … confinement …`
  (`crates/takt/src/run.rs:11634`) and the harness advisory after it (`:11652-11660`); the
  per-decision frontier (`crates/takt/src/run.rs:11743`).
- A story's case holds its title and body only in `intent.md`, "its title, a blank line, its body"
  (`crates/takt/src/run.rs:9599`); `events.jsonl` has no title or description
  (`Event::Opened`, `crates/takt/src/events.rs:110`).
- `takt run` has no `--verbose` flag today (`RunArgs`, `crates/takt-cli/src/main.rs:643`).
- Tests that pin the output: one assertion reads the per-decision frontier line,
  `"frontier: gate.run (admissible)"` (`crates/takt-cli/src/main.rs:4593`), and one unit test
  reads `frontier_line()` (`crates/takt/src/case.rs:4307`). Two read lines by position: the last
  line is the `stopped:` line (`crates/takt-cli/tests/protocol_defaults_cli.rs:570`) and a line
  starting `case ` and holding ` on ` follows the red-base declaration
  (`crates/takt-cli/tests/scenario_red_base_adv1.rs:404`). About 230 assertions read the prefixes
  `frontier: took`, `step N: <action>`, `  said:`, `  effect:` and `stopped:`; this story keeps
  them and only adds lines.
- `events.jsonl` keeps its `frontier` field as it is: `crates/takt/src/replay.rs:314` parses it.
  ESS declares no console output; it names "the frontier as printed" only in comments
  (`ess/domains/takt.yaml:935`, `:1507`, `:1883`).

## Acceptance

Observed by `crates/takt-cli/tests/run_console_cli.rs`:

1. A run of a story prints, before its first step, a header with the story's id, its title and
   at most three lines of its `## Outcome`; a run with `--intent` prints the intent's first line
   as the title.
2. Without `--verbose`, no line starting `frontier: ` holds `(admissible)`; the header holds
   `may: ` with the first decision's admissible actions, and a later decision that changes the
   admissible set prints `may now: ` with the added and removed actions, and one that does not
   change it prints nothing. The predicate above gives `not_reproduced`.
3. With `--verbose`, each decision prints its full `frontier:` line as today
   (`crates/takt-cli/src/main.rs:4593` passes with `--verbose` added to its run).
4. With standard output not a terminal, the output holds no escape byte (`\x1b`).
5. The `frontier: took`, `case … on …` and `stopped:` lines keep their prefixes; the `stopped:`
   line stays the last line; `events.jsonl` is unchanged.

## Scope

- `crates/takt/src/console.rs`: new, the header and the frontier diff (inferred)
- `crates/takt/src/run.rs`: the per-decision frontier at `:11743`, the case line at `:11634`, the
  story's title and body at `:9599` (cited)
- `crates/takt/src/case.rs`: `frontier_line` at `:2721`, the admissible set it is built from (cited)
- `crates/takt-cli/src/main.rs`: `RunArgs` (`:643`) gains `--verbose`; `settings:` at `:2055`,
  `frontier: took` at `:3909`, the assertion at `:4593` (cited)
- `crates/takt-cli/tests/run_console_cli.rs`: new, started in wave 41 (cited)
