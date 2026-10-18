---
format: aep.planning-md/3
id: story:confinement-seam
kind: story
status: draft
title: Every child process of a run starts through a Confinement adapter
summary: The fifth seam with a none adapter; a refusing test adapter proves nothing escapes it
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:second-harness-adapter
- depends_on: story:forge-pull-request-evidence
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/confinement.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/git.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/confinement.rs
revision: 7
---
# Story: confinement-seam

## Outcome

An operator can see that every place a run spawns a process when this story lands, the test command, the harness child, git, the bot-route forge and the `aep` frontier, goes through one seam where confinement is applied, and that `none` is the adapter in use, so a confining adapter can be added without touching the loop or any adapter.

## Context

`docs/design/takt.md` § 3 Seams: `Confinement (none first; Substrate later)`; § 4 M4: "the five traits ... Substrate optional". Today processes are spawned in three places with `std::process::Command` directly: the harness (`crates/takt/src/harness.rs:142-163`), the tests validator (`crates/takt/src/validator.rs:69-73`) and git (`crates/takt/src/git.rs:10-16`). This story lands last among the code stories of the epic, after story:frontier-next-ready-story (the `aep` adapter in `crates/takt/src/frontier.rs`), story:forge-pull-request-evidence (the `b10x-gates` adapter in `crates/takt/src/forge.rs`), story:gate-validator (a `canon-engineering` child in `crates/takt/src/validator.rs` if the implementer took that route) and story:second-harness-adapter (a second harness program), so when it starts five sites spawn: the test command, the harness child, git, the forge and the frontier.

This story: a `Confinement` trait that every spawn goes through (program, arguments, working directory, environment in; a supervised child out), a `None` adapter that passes through unchanged, every spawn site existing when it lands routed through it, the adapter named in the first line the run prints and in its events, and `--confinement none` as the only accepted value. A test adapter that refuses every program proves the routing: with it, the run starts no process at all. After this story `crates/takt/src/confinement.rs` is the only module that names `std::process::Command`, and a test in the gate holds that line, so a spawn site added after this story goes through the seam because that test stays in the gate. A Substrate adapter is a later story, written once Substrate is available to this repository.

## Domain relations

- Validator → Action and Harness step → Action are the two places a run spawns on a step. Inferred from `crates/takt/src/run.rs:174` (validator) and `run.rs:211` (harness); git spawns around them, `run.rs:223`, `run.rs:242`, `run.rs:250`; the forge (story:forge-pull-request-evidence) and the frontier (story:frontier-next-ready-story) spawn from their adapters, inferred from those stories' Context.

## Scope

- new: `crates/takt/src/confinement.rs`
- `crates/takt/src/lib.rs` (the `confinement` module registered)
- `crates/takt/src/harness.rs`, `crates/takt/src/validator.rs`, `crates/takt/src/git.rs`, `crates/takt/src/forge.rs`, `crates/takt/src/frontier.rs` (spawn through the seam)
- `crates/takt/src/run.rs` (the adapter carried by the run)
- `crates/takt-cli/src/main.rs` (`--confinement none`)
- new: `crates/takt/tests/confinement.rs`

## Ordering

This story lands last among the code stories of the epic, so every spawn site it routes exists when it starts.

- depends_on story:second-harness-adapter: both edit `crates/takt/src/harness.rs` and `crates/takt-cli/src/main.rs`; the second harness program is a spawn site this story routes.
- depends_on story:forge-pull-request-evidence: both edit `crates/takt/src/git.rs`, `crates/takt/src/validator.rs`, `crates/takt/src/forge.rs`, `crates/takt-cli/src/main.rs` and `crates/takt/src/lib.rs`; the `b10x-gates` adapter is a spawn site this story routes.
- story:declared-loops and story:story-done-end-to-end come before this story through story:second-harness-adapter; `crates/takt-cli/src/main.rs`, `crates/takt/src/run.rs` and `crates/takt/src/lib.rs` are shared with them along that chain.
- story:workflow-schema-and-loader, story:run-record-and-story-evidence, story:merge-under-authority, story:gate-validator and story:frontier-next-ready-story come before this story through the same chain; `crates/takt/src/frontier.rs`, `crates/takt/src/validator.rs`, `crates/takt/src/run.rs`, `crates/takt/src/harness.rs`, `crates/takt/src/lib.rs` and `crates/takt-cli/src/main.rs` are shared with them along it, and the `aep` adapter of story:frontier-next-ready-story is a spawn site this story routes.

## Acceptance

With a test `Confinement` adapter that refuses every program, `takt run --harness none` starts no process at any spawn site existing when this story lands (the test command, the harness child, git, the bot-route forge, the `aep` frontier), whichever is reached first on the scratch crate or on a repository with a planning store, writes no evidence record and stops with the refusal named, and with the `none` adapter the scratch-crate run writes the same `test_result` record it writes without the seam.

## Checks

- No module but `crates/takt/src/confinement.rs` names `std::process::Command`; a test in `crates/takt/tests/confinement.rs` reads the crate's sources and fails on any other occurrence.
- The first line the run prints and the run's events name the confinement adapter in use.

## Out of Scope

Any actual confinement (a Substrate adapter); network policy; resource limits. Sites added after this story go through the seam because the check that no module but `crates/takt/src/confinement.rs` names `std::process::Command` stays in the gate.

## Ambiguities

- Whether git belongs behind the seam: `inferable` as recorded here, yes, because a confining adapter that lets git out of the sandbox would let the harness commit; the design names the harness and validators only.
