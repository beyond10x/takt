# Wave 05: Merge only under a recorded grant of repository.merge

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed** (unit merged, gate green, story implemented). Approval: the operator's standing approval for the waves of `epic:composable-agentic-work` (same commit boundary each wave; no push).

## Unit

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:merge-under-authority` | `vision:safe-agentic-coding` | Two runs on `protocols/story-delivery/1.yaml` with a forge double whose checks pass: the first, whose case holds no grant, stops at `ApprovalRequired (repository.merge)` with the pull request open and the double recording no merge; after `takt approve <case> repository.merge`, `takt run --case <id>` resumes that case, merges through the double, writes a `merge_observation` record with result `merg | crates/takt-cli/src/main.rs (cited), crates/takt/src/case.rs (cited), crates/takt/src/forge.rs (inferred), crates/takt/src/frontier.rs (inferred), crates/takt/src/run.rs (cited), crates/takt/src/validator.rs (cited), crates/takt/tests/merge_under_authority.rs (inferred) |

Selection path: `aep plan artifact waves --kind story --status draft --format json` (aep 0.74.0); this is the verb's first wave, taken whole: one unit, because every other draft story is ordered after it by a `depends_on` chain. A wave of one is a wave.

## The verb's three lists, verbatim

### Waves

- wave 1: `story:merge-under-authority` (inferred)
- wave 2: `story:run-record-and-story-evidence` (inferred)
- wave 3: `story:story-done-end-to-end` (inferred)
- wave 4: `story:workflow-schema-and-loader` (inferred)
- wave 5: `story:declared-loops` (inferred)
- wave 6: `story:second-harness-adapter` (inferred)
- wave 7: `story:confinement-seam` (inferred)
- wave 8: `story:week-of-daily-runs` (inferred)

### Collisions (52)

- story:confinement-seam × story:declared-loops: `crates/takt-cli/src/main.rs` (cited)
- story:confinement-seam × story:declared-loops: `crates/takt/src/run.rs` (cited)
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
| integration | `wave/2026-10-18-w5` | managed tree `takt-w5-int` | its `target/` | `~/.cache/takt-waves/2026-10-18-w5/int/scratch` |
| merge-under-authority | `impl/merge-under-authority` at b1d7fca, two adversary passes, merged into `wave/2026-10-18-w5` at 06c20f9 | managed tree `takt-w5-merge` | its `target/` | `~/.cache/takt-waves/2026-10-18-w5/merge/scratch` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most). Unit brief at `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit (this page, the story to `active`), the unit's commits, the merge into the integration branch, the closing store commit, the merge into `main`. No push, no tag, no release.

## Log

- 2026-10-18: opened under the standing approval; `story:merge-under-authority` moved to active.
- 2026-10-18: unit green at 8953dff (red run: 19 unresolved imports in the new merge lane; green: merge_under_authority 10, cli unittests 13→17; 123→137 cases; 361,159 tokens, 93 tool uses, 24 min per the harness notification). Deviations recorded: `authority.yaml` carries approver and instant beside the grant and Canon is handed the two keys `canon-authority/1` declares (Canon refuses unknown fields); the story's Ambiguities line on `denied` corrected on the story body.
- 2026-10-18: adversary pass 1 NEEDS-CHANGE (145 cases, 6 red; 10 findings, 2 blockers: a harness could grant itself the merge by writing `authority.yaml`; a completed case was resumed by every later run; 240,810 tokens, 74 tool uses, 21 min), recorded as `review-result:adversary-merge-pass-1`; correction 1 green at 95e86ae (147 cases): the case directory is the kernel's (a judgement step that touches it is refused and the files restored), a finished case is not open, a resumed case moves to the branch's HEAD, `takt approve` refuses an unknown capability, `--harness none` stops `ApprovalRequired`.
- 2026-10-18: adversary pass 2 NEEDS-CHANGE (157 cases, 8 red; 9 findings, 2 blockers with one cause: a resumed run restarted its sequence at 1, overwriting decisions and repeating record ids; 260,978 tokens, 49 tool uses, 21 min), recorded as `review-result:adversary-merge-pass-2`; correction 2 green at b1d7fca (159 cases): one sequence per case across runs, the whole `.engineering/cases` guarded, a run holds its case (`run.lock`; `takt approve` and a second run refuse while held), symbolic links never followed, an unreadable decision leaves the case open, the cases exemption names the directory only. One pass-2 case rewritten to the lock rule.
- 2026-10-18: coordinator verification on b1d7fca (`~/.cache/takt-waves/2026-10-18-w5/int/scratch/verify-treatment.txt`): 159 cases (takt 123, takt-cli 36), clippy, fmt, `ess specify validate`, every exit 0; baseline on the base: no `merge_under_authority` test target. Merged into `wave/2026-10-18-w5`. Domain document: the grant and its record, the resume and the open-case meaning, the fifth producer `takt.pr-merged`, the lock, one sequence per case, the cases-directory refusal; citations re-pointed. Story scope rewritten, `test_result` recorded, moved to implemented. Follow-ups: a kernel-side stop at the gate without a judgement step (today the harness requests it); validator-owned actions offered to the harness when no validator is registered; the merge not yet proven against a live repository. Closed.
