---
format: aep.planning-md/3
id: story:baseline-runner-in-repo
kind: story
status: draft
title: The four-arm baseline runner is a Rust binary in the repository
summary: takt-bench arm runs one arm of one scenario, arm T on the takt binary --takt names, and its usage shim records each Claude call's raw usage, so any figure in docs/metrics can be rerun
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/takt-bench/Cargo.toml
- confidence: inferred
  path: crates/takt-bench/src/bin/usage-shim.rs
- confidence: inferred
  path: crates/takt-bench/src/main.rs
- confidence: inferred
  path: crates/takt-bench/tests/arm.rs
- confidence: inferred
  path: docs/metrics/README.md
revision: 13
---
# Story: baseline-runner-in-repo

## Outcome

The comparison method (four arms per scenario, one task text for the free arms, a held-out acceptance check, raw usage per run, timestamped streams) lives in the repository as a Rust binary, `takt-bench` (crate `crates/takt-bench`, package `b10x-takt-bench`, clap derive), documented in `docs/metrics/README.md`, so a reader can rerun any figure in docs/metrics from its commands.

- `takt-bench arm <arm> <i>` runs one arm of one scenario once and writes its `result.json`. Arm T runs the takt binary named by `--takt <path>`; the runner builds and finds none itself.
- The usage shim, a second binary of the same crate, stands first on a Claude arm's PATH under the name `claude`. It passes every argument to the real `claude` and appends the call's JSON result, unchanged, with `started_at` and `recorded_at`, to the run's `steps.jsonl`, the file `takt trace --steps` reads (`crates/takt/src/trace.rs:12-16`).

## Context

The runner of 2026-10-18, `run-arm.sh` with its shim `shim/claude` (docs/runs/2026-10-18-efficiency-analysis.md:30), lives outside the repository in the operator's scratch directory, so the figures in docs/metrics cite sources nobody else can run. That copy hard-codes a home directory, an author address and an ssh remote. The binary here takes the remote, the seed, the task text, the held-out check, the takt binary (`--takt`) and the model from arguments or the environment, and names no person or home path.

Running code committed to a beyond10x repository is Rust with clap derive, so the shell runner is rewritten, not copied. It is a crate of its own rather than a `takt` subcommand, so `crates/takt-cli/src/main.rs`, which many stories land on, stays out of its scope; the workspace's `members = ["crates/*"]` (`Cargo.toml:3`) takes it in. A crate is a judge surface (AGENTS.md § Rules), and its merge is granted as one.

"Reproduce" means the same fields from the same commands, not the same figures. A model is not deterministic. A run on scenario 1 also pushes a trial branch and calls the live harnesses (docs/runs/2026-10-18-efficiency-analysis.md § 2), so a real run is live and stays out of the acceptance. The offline check replaces the harnesses, the forge, the remote and takt itself with doubles and a local bare repository. The doubles takt's tests build today are `b10x-gates`, `gh` and `claude` (`crates/takt-cli/tests/scenario_run.rs:191-300`); `codex` and `takt` doubles are new work of this story.

Where arm T's takt comes from. A test in `crates/takt-bench` has no `CARGO_BIN_EXE_takt`, which Cargo sets only for the tests of the package that builds the binary, `b10x-takt-cli`. `task test-library`, which runs this crate's tests, excludes that package (`Taskfile.yml:32-35`). So the offline test passes a `takt` double through `--takt`. A run against the real binary is a documented command in `docs/metrics/README.md` (`cargo build --locked -p b10x-takt-cli`, then `takt-bench arm T <i> --takt target/debug/takt --out <out>`), live and not a test.

Why the check for personal data is the crate's own. The `b10x-gates` double takt's tests put first on PATH answers `scan-text` with exit 0 (`crates/takt-cli/tests/cases_under_dot_takt.rs:257`), so `b10x-gates scan-text` run inside a test observes nothing. The scan with the real policy is CI's `common / Security and privacy` job over the pull request's commits, outside this story's acceptance. The test reads the crate's own files instead (acceptance 3).

This story owns the edit to `docs/metrics/README.md`: the runner's commands, and the mapping of each `result.json` key to the row field it feeds (§ `scenario-runs.jsonl`). story:scenario-corpus-realistic documents its checks in `docs/metrics/scenarios/README.md` and does not edit `docs/metrics/README.md`.

## Expected before

On takt at 338082d, holds; after the change, fails:

- `ls crates/takt-bench` exits non-zero.
- `grep -c 'name = "b10x-takt-bench"' Cargo.lock` prints 0.

## Scope

- `crates/takt-bench/Cargo.toml`, `crates/takt-bench/src/main.rs`: the crate, the `arm` subcommand and its `--takt` flag (inferred)
- `crates/takt-bench/src/bin/usage-shim.rs`: the usage shim (inferred)
- `crates/takt-bench/tests/arm.rs`: the offline test, with the `codex` and `takt` doubles new (inferred)
- `Cargo.lock`: the new package (inferred)
- `docs/metrics/README.md`: the runner's commands, the live run of arm T against the real binary, and the `result.json` mapping (inferred)

## Acceptance

In `crates/takt-bench/tests/arm.rs`, run by `task test-library`, with doubles of `claude`, `codex`, `gh`, `b10x-gates` and `takt` built at test time, the first four first on PATH and the `takt` double passed as `--takt`, and a local bare clone of a fixture scratch repository as the remote:

1. For every arm of T (takt), C (Claude Code, free), I (Claude Code with takt's isolation flags) and X (Codex), `takt-bench arm <arm> 1 --takt <the takt double> --out <out>` exits 0 and writes `<out>/<arm>-1/result.json` holding the keys `arm`, `i`, `trial`, `branch`, `wall_seconds`, `last_exit`, `resumes`, `acceptance` and `pull_requests`. For T, the `takt` double's recorded argument list starts with `run`; for C, I and X, the `takt` double records no call. `takt-bench arm T 1` without `--takt` exits 2.
2. For C and I, `<out>/<arm>-1/steps.jsonl` holds one line per call the `claude` double answered, each with `started_at`, `recorded_at` and the double's usage object unchanged.
3. Every file under `crates/takt-bench` (read from `CARGO_MANIFEST_DIR`) holds no `/home/`, no `/Users/`, no `git@` remote and no e-mail address. The test's own control: the same matcher finds each of the four in a fixture string the test assembles at run time, so the test file holds none of them literally.

Before the change `crates/takt-bench` does not exist.
