---
format: aep.planning-md/3
id: story:work-flag-and-the-aep-source
kind: story
status: implemented
title: takt run --work names the item, and the aep source is the planning store behind it
summary: --work <key>, the ordered work list, aep as today, case ids for tracker keys
relations:
- decomposes: epic:work-sources
- serves: vision:safe-agentic-coding
- depends_on: story:work-items-in-the-ess-model
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/commit_style.rs
- confidence: cited
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T13:11:51Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-06T13:11:51Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-11-06T20:55:01Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Story: work-flag-and-the-aep-source

## Outcome

`takt run --work <key>` names the item a run works on, and the run file's `work` key lists the sources the next ready
item comes from, in order. The `aep` source is today's planning store behind the new noun: `--work aep:story:x` behaves
as `--story story:x` does (the story must be ready; its open case is resumed first), and `work = ["aep"]` is the
default. The case records the item's key; the `Refs:` trailer carries the full key for every source but `aep`, whose
trailer stays `Refs: <story id>`.

## Context

- Work enters through `trait Frontier` and `Unit` (`crates/takt/src/frontier.rs`); `Aep` is its only implementation.
  `--story` is checked by the `Named` frontier and resumed through `case_to_resume` (`crates/takt-cli/src/main.rs`).
- A key with another source than `aep` is refused in this story, naming the sources this release serves.
- The run file is flat keys under `deny_unknown_fields` (`crates/takt/src/run_config.rs`); `work` follows the
  `forge_instance` pattern.

## Case id for a tracker key

`aep` keeps `story-<name>-<instant>`. Any other source mints `<source>-<slug>-<h8>-<instant>`: the slug is the key's
connection and id (`team-a-abc-12` for `jira@team-a:ABC-12`), lowercased, each run of characters outside `[a-z0-9_]` one
`-`, at most 48 characters; `<h8>` is the first 8 hex digits of the sha256 of the full key, always
(approval-record:wave-56-work-model-points, point 9). `jira:ABC-12` is `jira-abc-12-<h8>-<instant>`;
`github:owner/repo#42` is `github-owner-repo-42-<h8>-<instant>`. The id passes `check_id`, and two keys never share an
id. The full key is recorded in the case beside the id.

## Acceptance

- `--work aep:<id>` and `--story <id>` take, resume and refuse the same story the same way: a test each.
- `--work jira:ABC-12` on this release exits 1 before any case opens, naming the sources it serves.
- `work = ["aep"]` in the run file, and no `work` key, select the next ready story as today: a test; an unknown source
  name in `work` is refused, naming it.
- A case minted from a tracker key has an id that passes `check_id` and differs for keys differing only in case or in a
  mapped character: a unit test.
- The case records the key; the commit trailer carries it.

## Scope

- `crates/takt/src/frontier.rs`, `crates/takt/src/run_config.rs`, `crates/takt/src/case.rs`,
  `crates/takt/src/commit_style.rs`, `crates/takt-cli/src/main.rs`; new tests in `crates/takt-cli/tests/`.

## Status

Built in wave 56 (unit b, 7361507b1), released in 0.34.0 from 2c4f1d7f4, the merge of pull request 63.

- `crates/takt/src/frontier.rs`: `Key` parses `<source>[@<connection>]:<id>` and mints tracker case ids; since
  a19b058f2 it refuses characters that do not show as themselves and an id starting with `-`.
- `crates/takt-cli/src/main.rs`: `--work <key>`; `aep:<story id>` shares `--story`'s path. The run file's `work` list
  (`crates/takt/src/run_config.rs`) defaults to `["aep"]`, also for a command line built without a run file
  (58723b3e3).
- The case records `item_key` and `item_revision`; `commit_style::reference` gives the trailer.
- `crates/takt-cli/tests/run_work_cli.rs` (red, then green).

Review: review-result:w56-adv-1.
