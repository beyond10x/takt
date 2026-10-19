---
format: aep.planning-md/3
id: story:bench-free-arms
kind: story
status: draft
title: The same benchmark tasks run under the free-running harnesses, for comparison
summary: Claude Code, Claude Code isolated and Codex on the same pinned tasks, so a takt result is a comparison
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:harbor-agent-adapter
- depends_on: story:swe-bench-mini-runner
- depends_on: story:baseline-runner-in-repo
scope:
- confidence: inferred
  path: docs/metrics/bench-runs.jsonl
revision: 7
---
# Story: bench-free-arms

## Outcome

Each pinned benchmark subset runs under the free-running arms of the baseline method as well (Claude Code with its operator configuration, Claude Code isolated as takt isolates it, Codex), using the benchmarks' own agents where they exist, so each takt result has its comparison.

## Context

In the scenario 1 comparison, isolation explained most of the earlier token gap (docs/metrics/README.md). A benchmark result without the isolated arm would repeat that mistake.

Of the three free arms, only Claude Code and Codex in their operator configuration can reuse an agent a benchmark ships, and only where Harbor ships one. Check this against Harbor's documentation when the story starts. SWE-bench ships no agent for either. "Claude Code isolated as takt isolates it" exists nowhere: no Harbor agent and no SWE-bench agent runs it. So this story writes one launcher, `docs/bench/run-free-arm.sh <arm> <task dir> <out>`, for all three arms on the tasks of both benchmarks. It is cut from the arm code of the runner in the repository (story:baseline-runner-in-repo). The isolated arm has to stay identical to takt's harness isolation, so the acceptance checks exactly that.

The round itself is live. Running the three arms on each task id of the pinned subsets (story:bench-subset-pinned) needs Docker, the datasets and model logins, so it is not part of the acceptance. Its rows are recorded under the schema of story:bench-runs-schema and drawn by story:bench-runs-in-journey. The journey view today reads only `scenario-runs.jsonl` (`docs/metrics/journey.py:21`) and only scenario 1 (`docs/metrics/journey.py:811`), which is why drawing the rows is a separate story.

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/bench/run-free-arm.sh` exits non-zero.

## Scope

- `docs/bench/run-free-arm.sh` (inferred)
- `docs/bench/fixtures/` (inferred)

## Acceptance

With a `claude` double first on PATH that writes its argument list to a file, `docs/bench/run-free-arm.sh claude-code-isolated <fixture task> <out>` starts `claude` with the four arguments of `ClaudeCode::ISOLATION` (`crates/takt/src/harness.rs:110-115`), where at d17bc32 `docs/bench/run-free-arm.sh` does not exist.
