# Wave 08: pending checks, and the intake of findings

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Approval: the operator role approved this selection (`ci-pending-waits` first, then `finding-intake`); each judge-surface merge needs the operator role's grant of its exact commit. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:ci-pending-waits` | `vision:safe-agentic-coding` | pending CI or a lagging pull-request head is polled within the run's budget; a run out of budget stops as waiting, not `NoProgress`; a merge the forge refuses for a pending required check is nothing merged | `crates/takt/src/run.rs`, `crates/takt/src/validator.rs` (cited); `crates/takt/tests/ci_pending_waits.rs` (inferred) | yes |
| `story:finding-intake` | `vision:safe-agentic-coding` | `scripts/file-finding` files a finding once per key; `scripts/check-finding` prints `reproduced`/`not_reproduced`; the operator profile carries the intake procedure | `scripts/file-finding`, `scripts/check-finding`, `.agents/operator-agent.md` (cited); `ess/domains/takt.yaml` (inferred) | yes (`.agents/**`, `scripts/**`) |

## Selection path

`aep plan artifact waves --kind story --status draft --format text` (aep 0.74.0): 14 waves, 134 collisions, 0 unassessed. Wave 1: `approval-riders-are-grants`, `case-scoped-step-guard`, `finding-intake`; wave 2: `ci-pending-waits`. The operator role's order puts `ci-pending-waits` first (it blocks unattended scenario runs, and two wave 07 cases name it); the verb records no collision between `ci-pending-waits` and `finding-intake`, so both run now.

## Left out

- `story:approval-riders-are-grants` and `story:case-scoped-step-guard`: collide with `ci-pending-waits` (`case.rs`, `run.rs`); next wave, as the operator role scheduled.
- `story:run-record-and-story-evidence`, then `story:scorecard-reader`: after `ci-pending-waits` (`run.rs`).

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 2a22093 |
| model budget | not stated; N = 2 |
| agent file | `AGENTS.md` read |
| gates policy for `scan-text` | the bot route's (`--policy` or `B10X_GATES_POLICY`) |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `wave/2026-10-18-w8` | managed tree `wave-08-integration` | its `target/` | `~/.cache/takt-wave-08/scratch/int` |
| ci-pending-waits | `impl/ci-pending-waits` | managed tree `w8-ci-pending-waits` | its `target/` | `~/.cache/takt-wave-08/scratch/ci-pending-waits` |
| finding-intake | `impl/finding-intake` | managed tree `w8-finding-intake` | its `target/` | `~/.cache/takt-wave-08/scratch/finding-intake` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) for each unit. Every merge into `wave/2026-10-18-w8` waits for the operator role's grant of that exact commit.

## The commits this wave makes

The opening store commit, each unit's commits, the merges into `wave/2026-10-18-w8` (each under a grant), the closing store commit, the merge into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: opened on the operator role's approval; both stories moved to active.
