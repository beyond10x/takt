# Wave 07: first wave of the self-improvement epic

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Opened on the standing approval; the selection was sent to the operator role. Approval: the operator role approved the waves of `epic:recursive-self-improvement` as a standing approval, with the owner's delegation; each judge-surface merge still needs its own grant from the operator role for the exact commit. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:agent-profiles` | `vision:safe-agentic-coding` | two harness-neutral profiles, operator and developer, under `.agents/`, with pointer stubs under `.claude/agents/`; the "never" lines copied word for word from the design; text passes `b10x-gates scan-text` | `.agents/operator-agent.md`, `.agents/developer-agent.md` (cited); `.claude/agents/*`, `AGENTS.md` (inferred) | yes (`.agents/**`, `.claude/**`) |
| `story:scenario-corpus` | `vision:safe-agentic-coding` | a scenario run cuts `trial/<n>` from the seed commit, pushes it through the bot route, checks it out and runs; the pull request targets `trial/<n>` | `crates/takt-cli/src/main.rs`, `crates/takt/src/git.rs`, `crates/takt-cli/tests/scenario_run.rs` (inferred) | yes (code) |
| `story:approval-record-reaches-base` | `vision:safe-agentic-coding` | after a merge under a grant, the base's planning store holds the approval record; the merged revision is still the granted one | `crates/takt/src/frontier.rs`, `crates/takt/src/validator.rs` (inferred) | yes (code) |

## Selection path

`aep plan artifact waves --kind story --status draft --format text` (aep 0.74.0): 13 waves, 141 collisions, 0 unassessed. The verb's wave 1 is `story:agent-profiles`, `story:approval-record-reaches-base`, `story:case-scoped-step-guard`; its wave 2 holds `story:scenario-corpus` and `story:run-record-and-story-evidence`.

**Disagreement with the verb, reported:** the operator role set the epic's order as `agent-profiles` and `scenario-corpus` first. This wave takes `agent-profiles`, `scenario-corpus` and `approval-record-reaches-base`, which are pairwise disjoint in the verb's own collision list (no collision line names two of them). `story:case-scoped-step-guard` stays out: the verb records `collision: story:case-scoped-step-guard story:scenario-corpus crates/takt/src/git.rs (inferred)`.

## Left out

- `story:case-scoped-step-guard`: collides with `scenario-corpus` on `git.rs`; next wave.
- Every other RSI story depends on a story not yet implemented (graph), or, for `story:loop-scorecard`, waits for three recorded L1 turns.

## Ordering note for scenario-corpus

The story's first step is to establish that the bot route admits a `trial/<n>` base on the scratch repository. The checks need the network (read-only `gh api` for rulesets and branches, the App's permissions, the live push and pull-request proof). The implementor builds and tests against stand-ins; the operator role runs the live proof, as it runs trials.

## Pre-flight

| check | value |
|---|---|
| free disk | 125G on `/` |
| tree | `main` at 92b8cf3, clean; wave 06's trees removed except `w6-int-fixes`, held by another process |
| model budget | not stated; N = 3 |
| agent file | `AGENTS.md` read (rewritten in wave 06) |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `wave/2026-10-18-w7` | managed tree `wave-07-integration` | its `target/` | `~/.cache/takt-wave-07/scratch/int` |
| agent-profiles | `impl/agent-profiles` | managed tree `w7-agent-profiles` (planned) | its `target/` | `~/.cache/takt-wave-07/scratch/agent-profiles` |
| scenario-corpus | `impl/scenario-corpus` | managed tree `w7-scenario-corpus` (planned) | its `target/` | `~/.cache/takt-wave-07/scratch/scenario-corpus` |
| approval-record-reaches-base | `impl/approval-record-reaches-base` | managed tree `w7-approval-record` (planned) | its `target/` | `~/.cache/takt-wave-07/scratch/approval-record` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) for the two code units; `aep:implementor` for `agent-profiles`, verified by the coordinator against the design's "never" lines and `scan-text`. Every merge into `wave/2026-10-18-w7` that touches a judge surface waits for the operator role's grant of that exact commit.

## The commits this wave makes

The opening store commit (this page, the Scope sections, the stories to `active`), each unit's commits, the merges into `wave/2026-10-18-w7` (each judge-surface merge under a grant), the closing store commit, the merge into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: proposed; the selection sent to the operator role; opened on the standing approval; the three stories moved to active.
