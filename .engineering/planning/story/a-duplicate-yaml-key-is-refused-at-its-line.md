---
format: aep.planning-md/3
id: story:a-duplicate-yaml-key-is-refused-at-its-line
kind: story
status: active
title: A duplicate key in a YAML run configuration is refused at its line
tags:
- finding
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T19:06:34Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-28T19:06:34Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A YAML run configuration file that holds one key twice is refused at the duplicate's line, naming
the key, as a TOML file is.

## Context

- Adversary case `a_duplicate_key_is_refused_at_its_line_in_yaml_as_in_toml`
  (`crates/takt/tests/run_config_adv1.rs`), committed ignored in wave 37 (d7eb1d4): TOML names
  line 3; YAML is refused at line 1, the line its mapping starts, with the generic message of
  what the file holds (serde_yaml_ng reports a duplicate key at the mapping's start). The file is
  refused either way; only the line and the reason are wrong.
- Wave 38 re-pinned the case to today's behaviour; it names this story.

## Acceptance

1. That case asserts YAML names line 3 and the key `iterations`, as TOML does.
