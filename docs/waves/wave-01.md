# Wave 01: the story protocol and the domain model

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed** (both units merged, gate green, stories implemented).

## Units

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:story-delivery-protocol` | `vision:safe-agentic-coding` | a test opens a case on `protocols/story-delivery/1.yaml`, adds the five evidence records in order; each decision shows one more claim true; `accepted` legitimate only after `pr.merged` | cited: README.md, crates/takt/src/protocol.rs; inferred: crates/takt/tests/story_delivery.rs, protocols/story-delivery/1.yaml |
| `story:takt-domain-model` | `vision:safe-agentic-coding` | `ess specify validate --path ess` exits 0 on a document declaring Case, Run, Workflow, Step, EvidenceProducer and their relations, no `UNMAPPED:` left | cited: AGENTS.md; inferred: ess/domains/takt.yaml, ess/ess-inputs.yaml, ess/system.yaml |

Selection path: `aep plan artifact waves --kind story --status draft --format json` (aep 0.74.0); wave 1 is the verb's first wave, taken whole. Both units rest partly on inferred scope (new files). Left out: the other eleven draft stories, each ordered after wave 1 by a `depends_on` chain (frontier → gate → forge → merge → run-record → story-done-end-to-end → workflow → declared-loops → second-harness → confinement → week); none is ready while its predecessor is draft.

## The verb's three lists, verbatim

### Waves

- wave 1: `story:story-delivery-protocol` (inferred), `story:takt-domain-model` (inferred)
- wave 2: `story:frontier-next-ready-story` (inferred)
- wave 3: `story:gate-validator` (inferred)
- wave 4: `story:forge-pull-request-evidence` (inferred)
- wave 5: `story:merge-under-authority` (inferred)
- wave 6: `story:run-record-and-story-evidence` (inferred)
- wave 7: `story:story-done-end-to-end` (inferred)
- wave 8: `story:workflow-schema-and-loader` (inferred)
- wave 9: `story:declared-loops` (inferred)
- wave 10: `story:second-harness-adapter` (inferred)
- wave 11: `story:confinement-seam` (inferred)
- wave 12: `story:week-of-daily-runs` (inferred)

### Collisions (104)

- story:confinement-seam × story:declared-loops: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:declared-loops: `crates/takt/src/run.rs`
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/forge.rs`
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/git.rs`
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/lib.rs`
- story:confinement-seam × story:forge-pull-request-evidence: `crates/takt/src/validator.rs`
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt/src/frontier.rs`
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt/src/lib.rs`
- story:confinement-seam × story:frontier-next-ready-story: `crates/takt/src/run.rs`
- story:confinement-seam × story:gate-validator: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:gate-validator: `crates/takt/src/validator.rs`
- story:confinement-seam × story:merge-under-authority: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/forge.rs`
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/frontier.rs`
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/run.rs`
- story:confinement-seam × story:merge-under-authority: `crates/takt/src/validator.rs`
- story:confinement-seam × story:run-record-and-story-evidence: `crates/takt/src/frontier.rs`
- story:confinement-seam × story:run-record-and-story-evidence: `crates/takt/src/lib.rs`
- story:confinement-seam × story:run-record-and-story-evidence: `crates/takt/src/run.rs`
- story:confinement-seam × story:second-harness-adapter: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:second-harness-adapter: `crates/takt/src/harness.rs`
- story:confinement-seam × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:story-done-end-to-end: `crates/takt/src/run.rs`
- story:confinement-seam × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:week-of-daily-runs: `crates/takt/src/lib.rs`
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt/src/harness.rs`
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt/src/lib.rs`
- story:confinement-seam × story:workflow-schema-and-loader: `crates/takt/src/run.rs`
- story:declared-loops × story:forge-pull-request-evidence: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:frontier-next-ready-story: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:frontier-next-ready-story: `crates/takt/src/run.rs`
- story:declared-loops × story:gate-validator: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:merge-under-authority: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:merge-under-authority: `crates/takt/src/run.rs`
- story:declared-loops × story:run-record-and-story-evidence: `crates/takt/src/run.rs`
- story:declared-loops × story:second-harness-adapter: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:story-done-end-to-end: `crates/takt/src/run.rs`
- story:declared-loops × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:declared-loops × story:workflow-schema-and-loader: `crates/takt/src/run.rs`
- story:declared-loops × story:workflow-schema-and-loader: `crates/takt/src/workflow.rs`
- story:forge-pull-request-evidence × story:frontier-next-ready-story: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:frontier-next-ready-story: `crates/takt/src/lib.rs`
- story:forge-pull-request-evidence × story:gate-validator: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:gate-validator: `crates/takt/src/validator.rs`
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt/src/case.rs`
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt/src/forge.rs`
- story:forge-pull-request-evidence × story:merge-under-authority: `crates/takt/src/validator.rs`
- story:forge-pull-request-evidence × story:run-record-and-story-evidence: `crates/takt/src/lib.rs`
- story:forge-pull-request-evidence × story:second-harness-adapter: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:story-done-end-to-end: `crates/takt/src/case.rs`
- story:forge-pull-request-evidence × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:week-of-daily-runs: `crates/takt/src/lib.rs`
- story:forge-pull-request-evidence × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:forge-pull-request-evidence × story:workflow-schema-and-loader: `crates/takt/src/lib.rs`
- story:frontier-next-ready-story × story:gate-validator: `crates/takt-cli/src/main.rs`
- story:frontier-next-ready-story × story:merge-under-authority: `crates/takt-cli/src/main.rs`
- story:frontier-next-ready-story × story:merge-under-authority: `crates/takt/src/frontier.rs`
- story:frontier-next-ready-story × story:merge-under-authority: `crates/takt/src/run.rs`
- story:frontier-next-ready-story × story:run-record-and-story-evidence: `crates/takt/src/frontier.rs`
- story:frontier-next-ready-story × story:run-record-and-story-evidence: `crates/takt/src/lib.rs`
- story:frontier-next-ready-story × story:run-record-and-story-evidence: `crates/takt/src/run.rs`
- story:frontier-next-ready-story × story:second-harness-adapter: `crates/takt-cli/src/main.rs`
- story:frontier-next-ready-story × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:frontier-next-ready-story × story:story-done-end-to-end: `crates/takt/src/run.rs`
- story:frontier-next-ready-story × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:frontier-next-ready-story × story:week-of-daily-runs: `crates/takt/src/lib.rs`
- story:frontier-next-ready-story × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:frontier-next-ready-story × story:workflow-schema-and-loader: `crates/takt/src/lib.rs`
- story:frontier-next-ready-story × story:workflow-schema-and-loader: `crates/takt/src/run.rs`
- story:gate-validator × story:merge-under-authority: `crates/takt-cli/src/main.rs`
- story:gate-validator × story:merge-under-authority: `crates/takt/src/validator.rs`
- story:gate-validator × story:second-harness-adapter: `crates/takt-cli/src/main.rs`
- story:gate-validator × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:gate-validator × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:gate-validator × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:merge-under-authority × story:run-record-and-story-evidence: `crates/takt/src/frontier.rs`
- story:merge-under-authority × story:run-record-and-story-evidence: `crates/takt/src/run.rs`
- story:merge-under-authority × story:second-harness-adapter: `crates/takt-cli/src/main.rs`
- story:merge-under-authority × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:merge-under-authority × story:story-done-end-to-end: `crates/takt/src/case.rs`
- story:merge-under-authority × story:story-done-end-to-end: `crates/takt/src/run.rs`
- story:merge-under-authority × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:merge-under-authority × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:merge-under-authority × story:workflow-schema-and-loader: `crates/takt/src/run.rs`
- story:run-record-and-story-evidence × story:story-done-end-to-end: `crates/takt/src/run.rs`
- story:run-record-and-story-evidence × story:week-of-daily-runs: `crates/takt/src/lib.rs`
- story:run-record-and-story-evidence × story:workflow-schema-and-loader: `crates/takt/src/lib.rs`
- story:run-record-and-story-evidence × story:workflow-schema-and-loader: `crates/takt/src/run.rs`
- story:second-harness-adapter × story:story-done-end-to-end: `crates/takt-cli/src/main.rs`
- story:second-harness-adapter × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:second-harness-adapter × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:second-harness-adapter × story:workflow-schema-and-loader: `crates/takt/src/harness.rs`
- story:story-done-end-to-end × story:week-of-daily-runs: `crates/takt-cli/src/main.rs`
- story:story-done-end-to-end × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:story-done-end-to-end × story:workflow-schema-and-loader: `crates/takt/src/run.rs`
- story:week-of-daily-runs × story:workflow-schema-and-loader: `crates/takt-cli/src/main.rs`
- story:week-of-daily-runs × story:workflow-schema-and-loader: `crates/takt/src/lib.rs`

### Unassessed (0)

- none

## Pre-flight (2026-10-18)

| check | value |
|---|---|
| free disk | 53G of 848G on `/` |
| tree | `main` at 2683aa1, clean but one untracked draft file (git-ignored directory) |
| worktrees | one, the primary checkout |
| build directory | `target/` 866M in the primary checkout (M1 builds); each unit builds into its own tree's `target/` |
| build cache | sccache present, `RUSTC_WRAPPER` unset; set `RUSTC_WRAPPER=sccache` for the wave's builds |
| one measured build | `cargo build -p b10x-takt-cli` cold with git dependencies about 4 min, incremental under 1 min; `task check` 2-3 min |
| model budget | not stated by the operator; N = 2 (the wave has two units), under the default of 4 |
| ESS | `ess` 0.60.0 on PATH (pinned); `aep` 0.74.0; `worktree` 0.18.2 |
| agent file | `AGENTS.md` read: `task check`; managed worktrees; bot route for commits; public text rules |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `wave/2026-10-18-w1` at 1638755 (opening commit) | managed tree `takt-w1-int` (worktree registry id takt-w1-int) | its `target/` | `~/.cache/takt-waves/2026-10-18-w1/int/scratch` |
| story-delivery-protocol | `impl/story-delivery-protocol` at 3afbd23, 13 cases; pass 1 (7 fixed) and pass 2 (confirmed; 1 no-op, 1 fixed) recorded; VERIFIED; merged | managed tree `takt-w1-protocol` | its `target/` | `~/.cache/takt-waves/2026-10-18-w1/protocol/scratch` |
| takt-domain-model | `impl/takt-domain-model` at 531b0ef (corrections 1 and 2); pass 1 (9 findings) and pass 2 (4) recorded, all fixed; VERIFIED; merged | managed tree `takt-w1-domain` | its `target/` | `~/.cache/takt-waves/2026-10-18-w1/domain/scratch` |

Managed trees live under the worktree registry's trees directory for this repository; unit briefs at `<scratch>/brief.md`; coordinator lease `takt-w1-coordinator` on the integration tree.

## Dispatch

`aep:implementor` per unit, then `aep:adversary` per unit (two passes at most). Unit briefs: `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit on the integration branch (this page, the two stories moved to `active`), one commit per unit, two merges into the integration branch, the closing store commit (scope confirmations, evidence, moves), and the merge of the integration branch into `main`. No push, no tag, no release.

## Log

- 2026-10-18: stage 1 written; waiting for the operator's approval of the commits above.
- 2026-10-18: operator approved wave 01 and granted standing approval for the following waves of the epic (same commit boundary, no push). Opening commit 1638755; two implementors dispatched (`aep:implementor`).
- 2026-10-18: protocol unit green at e50efdb (red run: 0 passed 2 failed with the file absent; green: 2 passed); the implementor found Canon reads one kind with two results at one revision as unknown, so `pr.merged` comes from `merge_observation`; the two story bodies corrected in the store. Domain unit green at ee1adfd (`takt v1 — 2 file(s), valid`; red runs: no `ess/`, then no domain file). Adversaries dispatched (`aep:adversary`). Implementor cost: protocol 117,087 tokens, 42 tool uses, 8.5 min; domain 189,938 tokens, 69 tool uses, 17 min.
- 2026-10-18: protocol adversary pass 1 (145,094 tokens, 24 tool uses, 12 min): 7 findings, 2 red cases; correction 1 at 467a6ab (161,258 tokens, 22 tool uses, 5 min), 9 cases green; pass 2 dispatched. Domain adversary pass 1 (150,357 tokens, 51 tool uses, 11 min): confirmed, 2 warnings, 7 notes; correction 1 dispatched; the two story citation errors fixed in the store. Pass-1 adversary scratch holds a 608M build directory (protocol unit), to be removed at teardown.
- 2026-10-18 03:2xZ: domain correction 1 (239,646 tokens, 30 tool uses, 4.5 min), pass 2 (147,666 tokens, 48 tool uses, 7 min; confirmed, 1 warning, 3 notes), correction 2 (253,181 tokens, 13 tool uses, 1.3 min; the coordinator read the diff). Protocol pass 2 (135,497 tokens, 37 tool uses, 10 min; confirmed, 2 mutant killers). Merges d2884e6 (protocol) and 554e073 (domain). Gate on 554e073, each step its own exit status: ess specify validate 0; cargo fmt --check 0; cargo clippy -D warnings 0; cargo test --workspace 0 (5 + 2 + 13 tests, 0 failed). Scope sections rewritten from the confirmation tables; test_result evidence recorded; both stories implemented. Sub-agent cost of the wave: 10 runs, about 1.69 M tokens.
- Teardown: the adversaries' scratch holds two 608M build directories under the protocol unit's scratch (adversary-1/tree-copy/target, adversary-2/tree-copy/target), removed at teardown by exact path; 18 stale scratch trees under the protocol tree's target/tmp go with the tree.
