# Wave 02: Pull the next ready story from the planning store as the unit of a run

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed** (unit merged, gate green, story implemented). Approval: the operator's standing approval for the waves of `epic:composable-agentic-work` (same commit boundary each wave; no push).

## Unit

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:frontier-next-ready-story` | `vision:safe-agentic-coding` | On a repository whose planning store holds one `proposed` story with no blocker and no unmet `depends_on`, `takt run --repo R --harness none --iterations 1` opens a case whose `intent.md` is that story and leaves the story at status `active`, and on a store with no such story it opens no case and exits saying nothing is ready. | crates/takt-cli/src/main.rs (cited), crates/takt/src/frontier.rs (inferred), crates/takt/src/lib.rs (cited), crates/takt/src/run.rs (cited), crates/takt/tests/frontier.rs (inferred) |

Selection path: `aep plan artifact waves --kind story --status draft --format json` (aep 0.74.0); this is the verb's first wave, taken whole: one unit, because every other draft story is ordered after it by a `depends_on` chain. A wave of one is a wave.

## The verb's three lists, verbatim

### Waves

- wave 1: `story:frontier-next-ready-story` (inferred)
- wave 2: `story:gate-validator` (inferred)
- wave 3: `story:forge-pull-request-evidence` (inferred)
- wave 4: `story:merge-under-authority` (inferred)
- wave 5: `story:run-record-and-story-evidence` (inferred)
- wave 6: `story:story-done-end-to-end` (inferred)
- wave 7: `story:workflow-schema-and-loader` (inferred)
- wave 8: `story:declared-loops` (inferred)
- wave 9: `story:second-harness-adapter` (inferred)
- wave 10: `story:confinement-seam` (inferred)
- wave 11: `story:week-of-daily-runs` (inferred)

### Collisions (104)

- story:confinement-seam × story:declared-loops: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:declared-loops: `crates/takt/src/run.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/forge.rs` (inferred)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/git.rs` (cited)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/validator.rs` (cited)
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt/src/frontier.rs` (inferred)
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt/src/lib.rs` (inferred)
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt/src/run.rs` (cited)
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
- story:declared-loops × story:frontier-next-ready-story: `crates/takt-cli/src/main.rs` (cited)
- story:declared-loops × story:frontier-next-ready-story: `crates/takt/src/run.rs` (cited)
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
- story:forge-pull-request-evidence × story:frontier-next-ready-story: `crates/takt-cli/src/main.rs` (cited)
- story:forge-pull-request-evidence × story:frontier-next-ready-story: `crates/takt/src/lib.rs` (inferred)
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
- story:frontier-next-ready-story × story:gate-validator: `crates/takt-cli/src/main.rs` (cited)
- story:frontier-next-ready-story × story:merge-under-authority: `crates/takt-cli/src/main.rs` (cited)
- story:frontier-next-ready-story × story:merge-under-authority: `crates/takt/src/frontier.rs` (inferred)
- story:frontier-next-ready-story × story:merge-under-authority: `crates/takt/src/run.rs` (cited)
- story:frontier-next-ready-story × story:run-record-and-story-evidence: `crates/takt/src/frontier.rs` (inferred)
- story:frontier-next-ready-story × story:run-record-and-story-evidence: `crates/takt/src/lib.rs` (inferred)
- story:frontier-next-ready-story × story:run-record-and-story-evidence: `crates/takt/src/run.rs` (cited)
- story:frontier-next-ready-story × story:second-harness-adapter: `crates/takt-cli/src/main.rs` (cited)
- story:frontier-next-ready-story × story:story-done-end-to-end: `crates/takt-cli/src/main.rs` (inferred)
- story:frontier-next-ready-story × story:story-done-end-to-end: `crates/takt/src/run.rs` (inferred)
- story:frontier-next-ready-story × story:week-of-daily-runs: `crates/takt-cli/src/main.rs` (cited)
- story:frontier-next-ready-story × story:week-of-daily-runs: `crates/takt/src/lib.rs` (inferred)
- story:frontier-next-ready-story × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs` (cited)
- story:frontier-next-ready-story × story:workflow-schema-and-loader: `crates/takt/src/lib.rs` (inferred)
- story:frontier-next-ready-story × story:workflow-schema-and-loader: `crates/takt/src/run.rs` (cited)
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
| free disk | 165G on `/` |
| tree | `main` clean; the previous wave's trees finished, archived and removed; its unit branches deleted |
| build | each tree its own `target/`; `RUSTC_WRAPPER=sccache`; incremental builds under 1 min, `task check` 2-3 min |
| model budget | not stated by the operator; N = 1 |
| agent file | `AGENTS.md` read |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `wave/2026-10-18-w2` | managed tree `takt-w2-int` | its `target/` | `~/.cache/takt-waves/2026-10-18-w2/int/scratch` |
| frontier-next-ready-story | `impl/frontier-next-ready-story` at 6ef44b6 (corrections 1 and 2), 53 cases; pass 1 (4 findings) and pass 2 (6) recorded; VERIFIED; merged | managed tree `takt-w2-frontier` | its `target/` | `~/.cache/takt-waves/2026-10-18-w2/frontier/scratch` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most). Unit brief at `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit (this page, the story to `active`), the unit's commits, the merge into the integration branch, the closing store commit, the merge into `main`. No push, no tag, no release.

## Log

- 2026-10-18: opened under the standing approval; `story:frontier-next-ready-story` moved to active.
- 2026-10-18: unit green at 3a411bf; red run: unresolved module (exit 101); green: lib 14, frontier 4, loop 2, story_delivery 13, cli 3. Scope correction: the intent argument sits at main.rs:36-38, not :30-32. Adversary pass 1 dispatched (`aep:adversary`).
- 2026-10-18: adversary pass 1 (133,737 tokens, 42 tool uses, 12 min): the story was taken before the case was opened; correction 1 at 366735b (227,833 tokens, 26 tool uses, 6 min): the case first, a refused take removes it; 45 cases. Pass 2 dispatched. Pass-1 scratch holds a 702M mutant build directory, removed at teardown.
- 2026-10-18 04:2xZ: pass 2 (185,095 tokens, 66 tool uses, 15 min; confirmed: a pre-existing defect in CaseDir::open found by two red cases, two mutant blind spots now covered, two notes no-op); correction 2 at 6ef44b6 (277,706 tokens, 27 tool uses, 6 min; the coordinator read the diff). Merge 7be883c; the domain comment on the unit updated. Gate on e8c5e3a, each step its own exit status: ess specify validate 0; cargo fmt --check 0; cargo clippy -D warnings 0; cargo test --workspace 0 (53 tests, 0 failed). Scope rewritten and typed (case.rs and frontier_cli.rs added); test_result evidence; story implemented. Sub-agent cost of the wave: 5 runs, about 1.0 M tokens. Teardown: pass-1 and pass-2 scratch hold 702M and 835M mutant build directories, removed at teardown by exact path.
