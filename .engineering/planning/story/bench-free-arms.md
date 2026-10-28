---
format: aep.planning-md/3
id: story:bench-free-arms
kind: story
status: draft
title: The same benchmark tasks run under the free-running harnesses, for comparison
summary: takt-bench free-arm runs Claude Code, Claude Code isolated and Codex on the same pinned tasks, so a takt result is a comparison
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:harbor-agent-adapter
- depends_on: story:swe-bench-mini-runner
- depends_on: story:baseline-runner-in-repo
scope:
- confidence: inferred
  path: crates/takt-bench/Cargo.toml
- confidence: inferred
  path: crates/takt-bench/src/free_arm.rs
- confidence: inferred
  path: crates/takt-bench/src/main.rs
- confidence: inferred
  path: crates/takt-bench/tests/fixtures
- confidence: inferred
  path: crates/takt-bench/tests/free_arm.rs
- confidence: inferred
  path: docs/metrics/bench-runs.jsonl
revision: 12
---
# Story: bench-free-arms

## Outcome

Each pinned benchmark subset runs under the free-running arms of the baseline method as well (Claude Code with its operator configuration, Claude Code isolated as takt isolates it, Codex), using the benchmarks' own agents where they exist, so each takt result has its comparison. The arms the benchmarks do not ship run through `takt-bench free-arm`, a subcommand of the runner binary story:baseline-runner-in-repo creates.

## Context

In the scenario 1 comparison, isolation explained most of the earlier token gap (docs/metrics/README.md). A benchmark result without the isolated arm would repeat that mistake.

Of the three free arms, only Claude Code and Codex in their operator configuration can reuse an agent a benchmark ships, and only where Harbor ships one. Check this against Harbor's documentation when the story starts. SWE-bench ships no agent for either. "Claude Code isolated as takt isolates it" exists nowhere: no Harbor agent and no SWE-bench agent runs it.

So this story adds one subcommand, `takt-bench free-arm <arm> <task dir> --out <out>`, for all three arms on the tasks of both benchmarks. It reuses the arm code of `takt-bench arm` (story:baseline-runner-in-repo). Running code committed to a beyond10x repository is Rust with clap derive, so the launcher is a subcommand of that crate, not a shell script. The isolated arm has to stay identical to takt's harness isolation. So it takes its arguments from `takt::harness::ClaudeCode::ISOLATION` (`crates/takt/src/harness.rs:1931-1936`) through a dependency on the library, never from a copy, and the acceptance checks exactly that.

The round itself is live. Running the three arms on each task id of the pinned subsets (story:bench-subset-pinned) needs Docker, the datasets and model logins, so it is not part of the acceptance. Its rows are recorded in `docs/metrics/bench-runs.jsonl` under the schema of story:bench-runs-schema and drawn by story:bench-runs-in-journey. The journey view today reads only `scenario-runs.jsonl` (`docs/metrics/journey.py:26`) and only scenario 1 (`docs/metrics/journey.py:839`), which is why drawing the rows is a separate story.

## Ordering

`crates/takt-bench/src/main.rs` is also where story:scenario-corpus-realistic registers `controls`. Both follow story:baseline-runner-in-repo (`depends_on`). Each adds one subcommand in a module of its own, so they share only the subcommand list; `aep plan artifact waves` keeps them in different waves.

## Expected before

On takt at 338082d, holds; after the change, fails:

- `ls crates/takt-bench/src/free_arm.rs` exits non-zero.

## Scope

- `crates/takt-bench/src/free_arm.rs`: the `free-arm` subcommand (inferred)
- `crates/takt-bench/src/main.rs`: registering `free-arm` (inferred)
- `crates/takt-bench/Cargo.toml`: the dependency on `b10x-takt` for `ClaudeCode::ISOLATION`, if story:baseline-runner-in-repo did not add it (inferred)
- `crates/takt-bench/tests/free_arm.rs` and `crates/takt-bench/tests/fixtures/`: the offline test and its fixture task (inferred)
- `docs/metrics/bench-runs.jsonl`: the rows of the live round, outside the acceptance (inferred)

## Acceptance

In `crates/takt-bench/tests/free_arm.rs`, with a `claude` double first on PATH that writes its argument list to a file, `takt-bench free-arm claude-code-isolated <fixture task> --out <out>` exits 0 and starts `claude` with the four arguments of `ClaudeCode::ISOLATION`, in that order. `takt-bench free-arm claude-code` starts it with none of them. Before the change `crates/takt-bench/src/free_arm.rs` does not exist.
