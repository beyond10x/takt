---
format: aep.planning-md/3
id: story:a-scenario-declares-an-expected-red-base
kind: story
status: active
title: A scenario declares an expected red base
summary: red_base = expected in scenarios.conf lets takt scenario run go past a seed that is red on purpose
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T06:52:41Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-28T06:52:41Z", actor: "human:timo", revision: 4}
---
## Outcome

A scenario can declare that its seed is red on purpose: `red_base = expected` in its section of
`.engineering/scenarios.conf`. `takt scenario run` on such a scenario goes past the red base as
`--continue-past-red-base` does, without the flag, and prints that the red base was expected by
the scenario's declaration. A scenario without the key still stops `RedBase`, exit 3. The
run-a-story guide's trial section says how a red-seeded scenario runs.

## Context

- Live trial on takt 0.16.0, scenario 3 of todo-app-example (`story:empty-or-broken-file`): its
  seed is red on purpose (a test for an empty `todo.json` fails before the fix). Run 1 stopped
  `RedBase`, exit 3, and needed `takt run --case ... --continue-past-red-base` by hand.
- `Scenario::read` in `crates/takt-cli/src/main.rs` refuses every key but `story` and `seed`, so a
  declaration needs takt to read the new key; any other value of `red_base` is refused naming it.
- The run-a-story guide's trial section (`website/docs/guides/run-a-story.md`, "A trial on a
  GitHub repository") covers only scenario 1.

## Scope

- `crates/takt-cli/src/main.rs`: `Scenario` and `Scenario::read`, `scenario_run` (cited)
- `crates/takt-cli/tests/scenario_red_base_cli.rs`: new
- `website/docs/guides/run-a-story.md`, `website/docs/guides/scenario-run.md`: the key and a
  red-seeded trial (cited)

## Acceptance

1. `scenario_red_base_cli::an_expected_red_base_goes_on_without_the_flag`: a scenario declaring
   `red_base = expected` whose seed's tests fail runs past the first `tests.run` to a judgement
   step, no `RedBase` stop, and prints the declaration it went on by.
2. `scenario_red_base_cli::an_undeclared_red_base_still_stops`: the same seed without the key stops
   `RedBase`, exit 3.
3. `scenario_red_base_cli::another_red_base_value_is_refused`: `red_base = maybe` is refused
   before anything is cut, exit 1, naming the key and its one value.
4. The guide's trial section names `red_base = expected` and how scenario 3 runs.
