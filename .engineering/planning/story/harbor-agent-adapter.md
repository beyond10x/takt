---
format: aep.planning-md/3
id: story:harbor-agent-adapter
kind: story
status: draft
title: takt is a Terminal-Bench 2.0 agent through a Harbor adapter
summary: a Harbor adapter installs takt in the task container and runs the task's instruction under bench.task/1
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-task-protocol
- depends_on: story:takt-in-container
- depends_on: story:bench-subset-pinned
- depends_on: story:takt-static-binary
- depends_on: story:baseline-runner-in-repo
scope:
- confidence: inferred
  path: crates/takt-bench/src/main.rs
- confidence: inferred
  path: crates/takt-bench/tests/harbor_run.rs
- confidence: inferred
  path: docs/bench/harbor/fixtures/hello
revision: 9
---
# Story: harbor-agent-adapter

## Outcome

Terminal-Bench 2.0 tasks run with takt as the agent: a Harbor adapter installs takt in each task container, runs `takt run` on the task's instruction under `bench.task/1`, and leaves grading to the task's own test script.

## Context

Terminal-Bench 2.0 runs through the Harbor harness (`harbor run -d terminal-bench/terminal-bench-2 -a <agent>`). Each task has an instruction, a test script and an oracle solution (design:benchmark-integration, sources there).

The adapter has two parts:

- **The takt side**, `takt-bench harbor-run <task dir> <logs>`, a subcommand of the `takt-bench` binary (crate `crates/takt-bench`, story:baseline-runner-in-repo, clap derive). It is what the acceptance checks offline, and it replaces the planned `run-takt.sh`:
  - When the task directory is not a git repository, it runs `git init` and commits the starting tree, because takt commits implementation revisions.
  - It runs `takt run --protocol protocols/bench-task/1.yaml --forge none --intent <instruction>` with `--case-id scenario-tb2-<task>-trial-<n>` (story:bench-in-scorecard).
  - It passes the task's visible test command as `--test-cmd`, taken from the subset file (story:bench-subset-pinned), or `true` when the task exposes none. Terminal-Bench copies its grading tests in after the agent finishes, so many tasks have no visible test command at all. A row for such a task says so (story:bench-runs-schema).
  - It copies the case directory into the logs directory, which outlives the container. Check that Harbor keeps the agent's logs directory once the container is removed.
- **The Harbor agent**, which installs the static `takt` (story:takt-static-binary), `takt-bench` and the harness CLI in the task container, then calls `takt-bench harbor-run` with the task's instruction.

### Open question: the Harbor agent is a Python class

Running code committed to this repository is Rust with clap derive (approval-record:store-replan-2026-10-28). Harbor's documentation (docs.harborframework.com, Agents › Custom agents, read 2026-10-28) defines a custom agent as a Python class: `BaseInstalledAgent` ("when Harbor should install and run the agent CLI", implementing `install()` and `run()`) or `BaseAgent`, loaded with `-a module.path:ClassName`, "The module must be importable from the Harbor process". That page names no way to give an agent as a plain executable. Its ACP page shows a generic `acp` agent configured with a local registry-style `agent.json` (`registry_entry_path`), which would need takt or `takt-bench` to speak the Agent Client Protocol; whether an arbitrary ACP executable works without a registry entry is not stated there.

So the planned `takt_agent.py` cannot be written here as things stand, and no exception to the rule is assumed. Options, for the owner, before this story is proposed:

- A: the Harbor agent class lives outside this repository (in a benchmark-harness repository or upstream in Harbor), and this repository ships only `takt-bench harbor-run`;
- B: `takt-bench` speaks ACP and Harbor's generic `acp` agent drives it from a committed `agent.json`, if Harbor's ACP constructor admits a local command;
- C: an explicit owner exception for a Python agent class of a few lines that only shells out to `takt-bench harbor-run`.

Recommendation and default: A, because it keeps this repository Rust and needs no protocol work; the story is scoped to `takt-bench harbor-run` until the owner says otherwise.

The live run is not part of the acceptance. `harbor run` on the ten pinned task ids of story:bench-subset-pinned needs Docker, the dataset and a model login. Its verdicts come from the tasks' own tests, and they are recorded as rows of story:bench-runs-schema.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -rqs harbor-run crates/takt-bench/src || echo no-harbor-run` prints `no-harbor-run`.

## Scope

- `crates/takt-bench/src/main.rs`: the `harbor-run` subcommand (inferred)
- `crates/takt-bench/tests/harbor_run.rs` (inferred)
- `docs/bench/harbor/fixtures/hello/` (inferred)
- the Harbor agent: where it lives is the open question above

## Acceptance

With a `claude` double first on PATH, `takt-bench harbor-run <task dir> <logs>` (the command the adapter runs inside a task container) on the fixture task `docs/bench/harbor/fixtures/hello/` (an `instruction.md`, no git repository, no tests) leaves `<logs>/case/events.jsonl` ending in a `stop` event whose `stop` is `Completed (completed)`, checked in `crates/takt-bench/tests/harbor_run.rs`, where at fb3c3bc no such subcommand exists.
