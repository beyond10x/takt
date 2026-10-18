# Wave 03: A gate validator turns an engineering-gates run into gate_result evidence

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **2, dispatched** (updated as the unit moves). Approval: the operator's standing approval for the waves of `epic:composable-agentic-work` (same commit boundary each wave; no push).

## Unit

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:gate-validator` | `vision:safe-agentic-coding` | On a scratch repository whose `.engineering/gates.yaml` holds one assertion, a run with `--harness none` on a protocol declaring `gate.green` writes exactly one `gate_result` record about `implementation` at HEAD whose result is `pass` when the assertion is true and `fail` when it is false, the next decision document shows `gate.green` true or false accordingly, and an assertion that evaluates unk | Cargo.toml (cited), crates/takt-cli/src/main.rs (cited), crates/takt/Cargo.toml (cited), crates/takt/src/validator.rs (cited), crates/takt/tests/gate_validator.rs (inferred) |

Selection path: `aep plan artifact waves --kind story --status draft --format json` (aep 0.74.0); this is the verb's first wave, taken whole: one unit, because every other draft story is ordered after it by a `depends_on` chain. A wave of one is a wave.

## The verb's three lists, verbatim

### Waves

- wave 1: `story:gate-validator` (inferred)
- wave 2: `story:forge-pull-request-evidence` (inferred)
- wave 3: `story:merge-under-authority` (inferred)
- wave 4: `story:run-record-and-story-evidence` (inferred)
- wave 5: `story:story-done-end-to-end` (inferred)
- wave 6: `story:workflow-schema-and-loader` (inferred)
- wave 7: `story:declared-loops` (inferred)
- wave 8: `story:second-harness-adapter` (inferred)
- wave 9: `story:confinement-seam` (inferred)
- wave 10: `story:week-of-daily-runs` (inferred)

### Collisions (81)

- story:confinement-seam × story:declared-loops: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:declared-loops: `crates/takt/src/run.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/forge.rs` (inferred)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/git.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/validator.rs` (cited)
- story:confinement-seam × story:gate-validator: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:gate-validator: `crates/takt/src/validator.rs` (cited)
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
- story:declared-loops × story:gate-validator: `crates/takt-cli/src/main.rs` (cited)
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
- story:forge-pull-request-evidence × story:gate-validator: `crates/takt-cli/src/main.rs` (cited)
- story:forge-pull-request-evidence × story:gate-validator: `crates/takt/src/validator.rs` (cited)
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
- story:gate-validator × story:merge-under-authority: `crates/takt-cli/src/main.rs` (cited)
- story:gate-validator × story:merge-under-authority: `crates/takt/src/validator.rs` (cited)
- story:gate-validator × story:second-harness-adapter: `crates/takt-cli/src/main.rs` (cited)
- story:gate-validator × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:gate-validator × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:gate-validator × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
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
| free disk | 156G on `/` |
| tree | `main` clean; the previous wave's trees finished, archived and removed; its unit branches deleted |
| build | each tree its own `target/`; `RUSTC_WRAPPER=sccache`; incremental builds under 1 min, `task check` 2-3 min |
| model budget | not stated by the operator; N = 1 |
| agent file | `AGENTS.md` read |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `wave/2026-10-18-w3` | managed tree `takt-w3-int` | its `target/` | `~/.cache/takt-waves/2026-10-18-w3/int/scratch` |
| gate-validator | `impl/gate-validator` at 77beec1, implementor green (53→59 cases; 161,650 tokens, 58 tool uses, 14 min), adversary pass 1 running | managed tree `takt-w3-gate` | its `target/` | `~/.cache/takt-waves/2026-10-18-w3/gate/scratch` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most). Unit brief at `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit (this page, the story to `active`), the unit's commits, the merge into the integration branch, the closing store commit, the merge into `main`. No push, no tag, no release.

## Log

- 2026-10-18: opened under the standing approval; `story:gate-validator` moved to active.
- 2026-10-18: unit green at 77beec1 (red run: unresolved GateRun, exit 101; green: gate_validator 3, cli unittests 6, the rest unchanged); the implementor's loop patch (an empty observation is an idle step) applied by the coordinator on the unit branch before the adversary. Scope: the gate-block route dropped (the loader story's), b10x-canon-expr added for Truth. Adversary pass 1 dispatched. The domain document's producer lines (553, 193/748) and a main.rs citation (776) are stale: fixed at close.
