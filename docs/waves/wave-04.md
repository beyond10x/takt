# Wave 04: Open the pull request through the bot route and read its state as evidence

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **2, dispatched** (updated as the unit moves). Approval: the operator's standing approval for the waves of `epic:composable-agentic-work` (same commit boundary each wave; no push).

## Unit

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:forge-pull-request-evidence` | `vision:safe-agentic-coding` | With a forge test double reporting the pull request open and its checks passing, a run on `protocols/story-delivery/1.yaml` with `--harness none` and the tests and gate validators green writes a `pull_request` record with result `open` and a `ci_result` record with result `pass`, both about `implementation` at the pushed revision, and its decision shows `pr.opened` and `ci.green` true and `reposit | crates/takt-cli/src/main.rs (cited), crates/takt/src/case.rs (cited), crates/takt/src/forge.rs (inferred), crates/takt/src/git.rs (cited), crates/takt/src/lib.rs (inferred), crates/takt/src/validator.rs (cited), crates/takt/tests/forge.rs (inferred) |

Selection path: `aep plan artifact waves --kind story --status draft --format json` (aep 0.74.0); this is the verb's first wave, taken whole: one unit, because every other draft story is ordered after it by a `depends_on` chain. A wave of one is a wave.

## The verb's three lists, verbatim

### Waves

- wave 1: `story:forge-pull-request-evidence` (inferred)
- wave 2: `story:merge-under-authority` (inferred)
- wave 3: `story:run-record-and-story-evidence` (inferred)
- wave 4: `story:story-done-end-to-end` (inferred)
- wave 5: `story:workflow-schema-and-loader` (inferred)
- wave 6: `story:declared-loops` (inferred)
- wave 7: `story:second-harness-adapter` (inferred)
- wave 8: `story:confinement-seam` (inferred)
- wave 9: `story:week-of-daily-runs` (inferred)

### Collisions (70)

- story:confinement-seam × story:declared-loops: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:declared-loops: `crates/takt/src/run.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/forge.rs` (inferred)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/git.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/validator.rs` (cited)
- story:confinement-seam × story:merge-under-authority: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/forge.rs` (inferred)
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/frontier.rs` (inferred)
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/run.rs` (cited)
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/validator.rs` (cited)
- story:confinement-seam × story:run-record-and-story-evidence: `crates/takt/src/frontier.rs` (inferred)
- story:confinement-seam × story:run-record-and-story-evidence: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:run-record-and-story-evidence: `crates/takt/src/run.rs` (cited)
- story:confinement-seam × story:second-harness-adapter: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:second-harness-adapter: `crates/takt/src/harness.rs` (cited)
- story:confinement-seam × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:confinement-seam × story:story-done-end-to-end: `crates/takt/src/run.rs` (inferred)
- story:confinement-seam × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:week-of-daily-runs: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt/src/harness.rs` (cited)
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt/src/run.rs` (cited)
- story:declared-loops × story:forge-pull-request-evidence: `crates/takt-cli/src/main.rs` (cited)
- story:declared-loops × story:merge-under-authority: `crates/takt-cli/src/main.rs` (cited)
- story:declared-loops × story:merge-under-authority: `crates/takt/src/run.rs` (cited)
- story:declared-loops × story:run-record-and-story-evidence: `crates/takt/src/run.rs` (cited)
- story:declared-loops × story:second-harness-adapter: `crates/takt-cli/src/main.rs` (cited)
- story:declared-loops × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:declared-loops × story:story-done-end-to-end: `crates/takt/src/run.rs` (inferred)
- story:declared-loops × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:declared-loops × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:declared-loops × story:workflow-schema-and-loader: `crates/takt/src/run.rs` (cited)
- story:declared-loops × story:workflow-schema-and-loader: `crates/takt/src/workflow.rs` (inferred)
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt-cli/src/main.rs` (cited)
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt/src/case.rs` (cited)
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt/src/forge.rs` (inferred)
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt/src/validator.rs` (cited)
- story:forge-pull-request-evidence × story:run-record-and-story-evidence: `crates/takt/src/lib.rs` (inferred)
- story:forge-pull-request-evidence × story:second-harness-adapter: `crates/takt-cli/src/main.rs` (cited)
- story:forge-pull-request-evidence × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:forge-pull-request-evidence × story:story-done-end-to-end: `crates/takt/src/case.rs` (inferred)
- story:forge-pull-request-evidence × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:forge-pull-request-evidence × story:week-of-daily-runs: `crates/takt/src/lib.rs` (inferred)
- story:forge-pull-request-evidence × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:forge-pull-request-evidence × story:workflow-schema-and-loader: `crates/takt/src/lib.rs` (inferred)
- story:merge-under-authority × story:run-record-and-story-evidence: `crates/takt/src/frontier.rs` (inferred)
- story:merge-under-authority × story:run-record-and-story-evidence: `crates/takt/src/run.rs` (cited)
- story:merge-under-authority × story:second-harness-adapter: `crates/takt-cli/src/main.rs` (cited)
- story:merge-under-authority × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:merge-under-authority × story:story-done-end-to-end: `crates/takt/src/case.rs` (inferred)
- story:merge-under-authority × story:story-done-end-to-end: `crates/takt/src/run.rs` (inferred)
- story:merge-under-authority × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:merge-under-authority × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:merge-under-authority × story:workflow-schema-and-loader: `crates/takt/src/run.rs` (cited)
- story:run-record-and-story-evidence × story:story-done-end-to-end: `crates/takt/src/run.rs` (inferred)
- story:run-record-and-story-evidence × story:week-of-daily-runs: `crates/takt/src/lib.rs` (inferred)
- story:run-record-and-story-evidence × story:workflow-schema-and-loader: `crates/takt/src/lib.rs` (inferred)
- story:run-record-and-story-evidence × story:workflow-schema-and-loader: `crates/takt/src/run.rs` (cited)
- story:second-harness-adapter × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:second-harness-adapter × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:second-harness-adapter × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:second-harness-adapter × story:workflow-schema-and-loader: `crates/takt/src/harness.rs` (cited)
- story:story-done-end-to-end × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (inferred)
- story:story-done-end-to-end × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (inferred)
- story:story-done-end-to-end × story:workflow-schema-and-loader: `crates/takt/src/run.rs` (inferred)
- story:week-of-daily-runs × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:week-of-daily-runs × story:workflow-schema-and-loader: `crates/takt/src/lib.rs` (inferred)

### Unassessed (0)

- none

## Pre-flight

| check | value |
|---|---|
| free disk | 151G on `/` |
| tree | `main` clean; the previous wave's trees finished, archived and removed; its unit branches deleted |
| build | each tree its own `target/`; `RUSTC_WRAPPER=sccache`; incremental builds under 1 min, `task check` 2-3 min |
| model budget | not stated by the operator; N = 1 |
| agent file | `AGENTS.md` read |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `wave/2026-10-18-w4` | managed tree `takt-w4-int` | its `target/` | `~/.cache/takt-waves/2026-10-18-w4/int/scratch` |
| forge-pull-request-evidence | `impl/forge-pull-request-evidence` | managed tree `takt-w4-forge` | its `target/` | `~/.cache/takt-waves/2026-10-18-w4/forge/scratch` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most). Unit brief at `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit (this page, the story to `active`), the unit's commits, the merge into the integration branch, the closing store commit, the merge into `main`. No push, no tag, no release.

## Log

- 2026-10-18: opened under the standing approval; `story:forge-pull-request-evidence` moved to active.
