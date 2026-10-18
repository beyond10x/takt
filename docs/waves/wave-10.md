# Wave 10: steps that cannot forge, grants on the action's subject, the trial fetch, the scorecard

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Approval: the operator role approved this selection (option A: these four; `story:records-reach-base` in wave 11); each judge-surface merge needs the operator role's grant of its exact commit, and `grant-binds-to-action-subject` merges first. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:step-cannot-forge-its-validators` | `vision:safe-agentic-coding` | a judgement step that changes a validator's inputs (`.cargo/**`, the test command's script, `.github/**`, `build.rs`, `rust-toolchain*`, `.engineering/**` but cases) earns no `pass` or `ci.green`: it is refused and put back | `crates/takt/src/run.rs`, `crates/takt/src/validator.rs` (cited); `crates/takt-cli/src/main.rs`, a test (inferred) | yes |
| `story:grant-binds-to-action-subject` | `vision:safe-agentic-coding` | a grant binds to the revision of the artifact its action is about, read from the protocol; a `self.improvement/1` case is granted at its `candidate` | `crates/takt/src/case.rs` (cited); a test (inferred) | yes |
| `story:trial-fetch-needs-no-user-ssh` | `vision:safe-agentic-coding` | the trial fetch needs no user credential; one that cannot authenticate fails within 30 s naming the reason | `crates/takt-cli/src/main.rs` `TrialRoute`, `crates/takt/src/forge.rs` (inferred) | yes |
| `story:scorecard-reader` | `vision:safe-agentic-coding` | `takt scorecard` prints per run the tokens summed from its events, with `--save` and `--against` | new `crates/takt/src/scorecard.rs`, `crates/takt/src/lib.rs`, `crates/takt-cli/src/main.rs` (cited); a test (inferred) | yes |

## Selection path

The operator role's option A of three, against the scopes at local main f3a8b1f. Shared files: `crates/takt-cli/src/main.rs` (step-cannot-forge: where the test command reaches the guard; trial-fetch: `TrialRoute`; scorecard-reader: a new subcommand) and `crates/takt/src/run.rs` (step-cannot-forge: the step guard; scorecard-reader: only the `events.emit` call sites its event fields need). Each brief names its functions; the coordinator checks `git merge-tree` before each merge.

`grant-binds-to-action-subject` was started before this opening, from the wave 09 branch tip cc879cb (its scope, the grant functions of `case.rs`, was untouched by the unit still under review); it merges as part of this wave.

## Left out

- `story:records-reach-base`: `case.rs` and `validator.rs`, shared with grant-binds and step-cannot-forge; wave 11.
- `story:pr-opens-when-change-is-done`, with `story:no-model-step-for-approval` and `story:no-inspect-for-stateless-harness` as one unit: `run.rs` step choice; wave 11 by collision.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at b670264; `task check` exit 0 there (461 passed; 367 of 367 citations) |
| model budget | not stated; N = 2 |
| agent file | `AGENTS.md` read |
| gates policy for `scan-text` | the bot route's (`--policy` or `B10X_GATES_POLICY`) |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) per unit; the last correction is verified by the coordinator. Every merge into `wave/2026-10-18-w10` waits for its grant.

## The commits this wave makes

The opening store commit, each unit's commits, the merges into `wave/2026-10-18-w10` (each under a grant), the closing store commit, the merge into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: opened on the operator role's approval; the four stories moved to active.
