---
format: aep.planning-md/3
id: story:takt-lists-a-protocols-settings
kind: story
status: implemented
title: takt protocol show lists a protocol's inputs, defaults and the repository's values
summary: a listing, text and json
tags:
- priority:medium
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:protocol-settings-in-the-run-file
- depends_on: story:the-run-file-sets-protocol-settings
- depends_on: story:takt-ships-default-protocol-settings
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/tests/protocol_show.rs
- confidence: inferred
  path: website/docs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T12:00:45Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-27T12:00:45Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-28T19:03:31Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}, executor: "agent:planner"}
---
# Story: takt-lists-a-protocols-settings

## Outcome

`takt protocol show <name/rev>` prints each declared input of the protocol. For each input it shows:
- its values and its description;
- the built-in default;
- the value the repository's run file sets, with that file's path and line.

`--format json` gives the same as data. An unknown protocol exits 1, naming the protocols takt knows.

## Context

- The run file sets protocol inputs (`story:the-run-file-sets-protocol-settings`), and takt ships defaults
  (`story:takt-ships-default-protocol-settings`).
- Without a listing, an operator learns the settings only from protocol YAML.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `protocol_show::lists_inputs_defaults_and_the_run_file_value`: for `story.delivery/2` with a run file setting
   `gate_on: each_revision`, the output lists `tests_first` (values, default `required`, run file unset) and `gate_on`
   (default `after_handover`, run file `each_revision` with its line).
2. `protocol_show::json_matches_text`.
3. `protocol_show::an_unknown_protocol_names_the_known_ones`.

## Scope

- `crates/takt-cli/src/main.rs` (`protocol show` subcommand)
- `crates/takt/src/protocol.rs` (input listing)
- `crates/takt/tests/protocol_show.rs`
- `website/docs/` (generated CLI reference)
