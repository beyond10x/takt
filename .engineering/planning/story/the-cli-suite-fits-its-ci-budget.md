---
format: aep.planning-md/3
id: story:the-cli-suite-fits-its-ci-budget
kind: story
status: implemented
title: 'task check on CI finishes well inside its time limit again: the test binaries that slowed after wave 23 are found and made fast or split'
summary: 'check.yml on 6ad32d3 cut at 30 min: summed test time 839 s at 0.6.0 vs 1433 s at wave 23; self_improvement_cli +199 s first'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T08:13:39Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-21T08:13:39Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-29T21:30:01Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: the-cli-suite-fits-its-ci-budget

## Outcome

`task check` on CI finishes well inside its job's time limit again: the test binaries whose time
grew between 0.6.0 and wave 23 are found, each growth is attributed to its cause (more tests, a
slower harness path, a wait, the runner), and the ones takt made slower are made fast again or
split, so the summed test time on CI returns near its 0.6.0 level and the job never ends on its
timeout.

## Context

`check.yml` on main 6ad32d3 (wave 23's merge) was cancelled by its 30-minute job timeout
(`.github/workflows/check.yml:31`, `timeout-minutes: 30`), run 39449110044 attempt 1, 15:30:55 to
16:01:14 UTC on 2026-10-20: 1551 tests passed before the cut and 8 binaries were not reached. The
summed time CI reported per test binary, from the job logs:

| run | commit | binaries | summed time |
|---|---|---|---|
| 39403844729 (green) | fd395d2 (0.6.0) | 140 | 839 s |
| 39449110044 attempt 1 (cut) | 6ad32d3 (wave 23) | 136 reached | 1433 s |

Largest growth, per binary (fd395d2 → 6ad32d3): `self_improvement_cli` 155.4 → 354.4 s (+199),
`gate_compares_imports` 58.6 → 140.3 s (+82; wave 23 added 4 tests), `gate_validator` 47.7 →
116.3 s (+69), `scenario_run` 40.4 → 90.1 s (+50), `delivery_candidate_cli` 24.8 → 60.2 s (+35),
`takt_gates_document` 22.2 → 46.7 s (+25). Binaries wave 23 did not touch grew too
(`validator_confinement` 12.0 → 31.3 s, `forge_scan_policy` 0.2 → 17.1 s), so part of the growth
is likely the runner; that is inferred, not shown. Wave 23 changed `--harness claude` to run
through a metaharness session in a scratch copy (story:harness-proposes-takt-performs), which
every CLI test with a fake `claude` now takes. Comes from self-improvement work, so it carries
`rsi`.

Since wave 24 (main 0fab445): run 39483288396 attempt 2 ended on the same 30-minute limit
(19:43:07 to 20:13:26 UTC on 2026-10-20), every binary it reached green; the summed test-binary
time was 1401 s over 139 binaries against 839 s at fd395d2 (0.6.0). The job's limit moved to 45
minutes as a stopgap (149a4e2, the orchestrator's word); this story brings it back to 30. The
five slowest test binaries at 0fab445 against 0.6.0, from the job logs:

| binary | fd395d2 (0.6.0) | 0fab445 (attempt 2) |
|---|---|---|
| `self_improvement_cli` | 155.4 s | 354.6 s |
| `gate_compares_imports` | 58.6 s | 137.3 s |
| `gate_validator` | 47.7 s | 117.0 s |
| `scenario_run` | 40.4 s | 86.9 s |
| `hours_bounds_the_run_end` | 88.8 s | 74.0 s |

## Scope

- `crates/takt-cli/tests/self_improvement_cli.rs` first, then the next binaries by growth; the
  harness path they share (`crates/takt/src/harness.rs` `ClaudeSession`,
  `crates/session-executor/src/lib.rs`) where the time is spent there.

## Acceptance

1. A measured breakdown, on one machine, of `self_improvement_cli` at fd395d2 and at the current
   main: per test, its wall time, and for the slowest five where the time goes (the harness
   session, git, the gate's commands, waits), each figure from a command's output.
2. Each cause takt owns is fixed or split into its own CI job, with the before and after times;
   a cause that is the runner is named as such with the evidence.
3. `check.yml` on main finishes in under two thirds of 30 minutes for three
   consecutive runs, and the job's `timeout-minutes` is back at 30.
