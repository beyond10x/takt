---
format: aep.planning-md/3
id: story:baseline-runner-in-repo
kind: story
status: draft
title: The four-arm baseline runner is part of the repository
summary: run-arm.sh, the token shim and the acceptance convention move into the repository so any run can be reproduced
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: docs/metrics/runner
revision: 7
---
# Story: baseline-runner-in-repo

## Outcome

The comparison method (four arms per scenario, one task text for the free arms, a held-out acceptance check, raw usage per run, timestamped streams) lives in the repository as a documented runner, so a reader can reproduce any figure in docs/metrics from its commands.

## Context

The runner of 2026-10-18 lives outside the repository, in the operator's scratch directory, so the figures in docs/metrics cite sources nobody else can run. That copy hard-codes a home directory, an author address and an ssh remote. The copy in the repository takes the remote, the seed, the task text, the acceptance script, the takt binary and the model from arguments or the environment, and names no person or home path.

"Reproduce" means the same fields from the same commands, not the same figures. A model is not deterministic. A run on scenario 1 also pushes a trial branch and calls the live harnesses (docs/runs/2026-10-18-efficiency-analysis.md § 2), so a real run is live and stays out of the acceptance. The offline check replaces the harnesses, the forge and the remote with doubles and a local bare repository. `docs/metrics/README.md` § `scenario-runs.jsonl` maps each `result.json` key to the row field it feeds.

This is a docs-and-tooling move, kept out of the judged code paths: `docs/**` is not a judge surface (AGENTS.md § Rules).

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/metrics/runner` exits non-zero.

## Scope

- `docs/metrics/runner/run-arm.sh` (inferred)
- `docs/metrics/runner/test-run-arm.sh` (inferred)
- `docs/metrics/runner/fixtures/` (inferred)

## Acceptance

With doubles of `claude`, `codex`, `gh` and `b10x-gates` first on PATH (built as `crates/takt-cli/tests/scenario_run.rs:260-270` builds one) and a local bare clone of a fixture scratch repository as the remote, `docs/metrics/runner/run-arm.sh <arm> 1` writes `<out>/<arm>-1/result.json` holding the keys `arm`, `i`, `trial`, `branch`, `wall_seconds`, `last_exit`, `resumes`, `acceptance` and `pull_requests` for every arm of T (takt), C (Claude Code, free), I (Claude Code with takt's isolation flags) and X (Codex), where at d17bc32 `docs/metrics/runner/run-arm.sh` does not exist.
