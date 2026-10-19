---
format: aep.planning-md/3
id: story:every-yaml-read-refuses-aliases
kind: story
status: draft
title: Every YAML file takt reads refuses anchors and aliases
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:judge-guard
revision: 1
---
## Outcome

Every YAML document takt parses goes through the parser-event check story:loop-scorecard introduced (correction 11): an anchor or an alias is refused, naming the file and line, before serde builds anything from it. One reader serves every call site.

## Context

From the developer's report on story:loop-scorecard correction 11 (wave 13, 2026-10-19) and its grant review at 64dbe18: the loop scorecard's own reads are checked, but YAML reads without the check remain in `crates/takt/src/case.rs` (8 places, among them `story_of` at `:1031-1041`, which the loop reached), `crates/takt/src/protocol.rs` (2), `crates/takt/src/validator.rs:533` and `crates/takt/src/run.rs:1899`. The reviewer's probe: an alias bomb x32000 in `story.yaml` was accepted in 30 ms with +3 MiB; the same text in `case.yaml` is refused. A plain `&` or `*` inside a scalar stays accepted.

## Scope

- `crates/takt/src/case.rs`, `crates/takt/src/protocol.rs`, `crates/takt/src/validator.rs`, `crates/takt/src/run.rs`: the YAML reads (cited)
- `crates/takt/src/loop_scorecard.rs`: the event check, moved where every reader can call it (inferred)

## Acceptance

A `story.yaml`, `case.yaml`, `protocol.yaml` and gates document each holding `a: &x 1` / `b: *x` is refused naming its file and line; each still reads with `a: "&x"` and `b: "*x"`.
