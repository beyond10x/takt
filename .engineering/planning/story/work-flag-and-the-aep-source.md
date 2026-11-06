---
format: aep.planning-md/3
id: story:work-flag-and-the-aep-source
kind: story
status: draft
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
revision: 6
---
# Story: work-flag-and-the-aep-source

## Outcome

`takt run --work <key>` names the item a run works on, and the run file's `work` key lists the sources the next ready
item comes from, in order. The `aep` source is today's planning store behind the new noun: `--work aep:story:x` behaves
as `--story story:x` does (the story must be ready; its open case is resumed first), and `work = ["aep"]` is the
default. The case records the item's key; the `Refs:` trailer carries it.

## Context

- Work enters through `trait Frontier` and `Unit` (`crates/takt/src/frontier.rs`); `Aep` is its only implementation.
  `--story` is checked by the `Named` frontier and resumed through `case_to_resume` (`crates/takt-cli/src/main.rs`).
- A key with another source than `aep` is refused in this story, naming the sources this release serves.
- The run file is flat keys under `deny_unknown_fields` (`crates/takt/src/run_config.rs`); `work` follows the
  `forge_instance` pattern.

## Case id for a tracker key

`aep` keeps `story-<name>-<instant>`. Any other source mints `<source>-<slug>-<instant>`: the key's id part lowercased,
each run of characters outside `[a-z0-9._]` one `-`, at most 48 characters, and, when that loses anything (case, a
character mapped, a cut), `-` and the first 8 hex digits of the sha256 of the full key. `jira:ABC-12` is
`jira-abc-12-<h8>-<instant>`; `github:owner/repo#42` is `github-owner-repo-42-<h8>-<instant>`. The id passes
`check_id`, and two keys never share a slug. The full key is recorded in the case beside the id.

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
