# Wave 09: the run record, riding grants, delegation text, citations, scenario help

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Approval: the operator role approved this selection (`run-record-and-story-evidence` first, as the head of the self-improvement chain); each judge-surface merge needs the operator role's grant of its exact commit, except `judge-surface-delegation-text`, which merges on the owner's recorded decision (main at 3ad0bd9: `product-requirements:takt` § 2 and R6). Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:run-record-and-story-evidence` | `vision:safe-agentic-coding` | the case directory holds `events.jsonl` (one line per evaluation, step, evidence record and stop); a tests pass record on a story unit becomes a `test_result` record in the store at its `observed_at`; an accepted case moves the story to `implemented` | `crates/takt/src/run.rs` (cited); `events.rs`, `frontier.rs`, `lib.rs`, `tests/run_record.rs` (inferred) | yes |
| `story:approval-riders-are-grants` | `vision:safe-agentic-coding` | a commit riding on a granted revision adds only approval records matching `authority.yaml` grants, one per grant | `crates/takt/src/case.rs` (cited); a test (inferred) | yes |
| `story:judge-surface-delegation-text` | `vision:safe-agentic-coding` | `AGENTS.md` § Rules, the operator profile and both improvement protocols state delegation on record | `AGENTS.md` § Rules, `.agents/operator-agent.md`, `protocols/self-improvement/1.yaml`, `protocols/loop-improvement/1.yaml` (cited) | yes |
| `story:citation-check-in-task-check` | `vision:safe-agentic-coding` | `task check` fails on a stale `read from:` citation and passes on main | `Taskfile.yml`, `scripts/check-citations`, `AGENTS.md` Commands (inferred) | yes |
| `story:scenario-run-help-exit-codes` | `vision:safe-agentic-coding` | `takt scenario run --help` lists no `frontier: nothing ready`; `scripts/check-finding` gives `reproduced` at 0a776d2 and `not_reproduced` at the fix | `crates/takt-cli/src/main.rs`, the scenario run's `after_help` (inferred) | yes |

## Selection path

`aep plan artifact waves --kind story --status draft --format text` (aep 0.74.0) places these five in its waves 1, 2, 5 and 6, because it colours greedily across the whole draft set. Pairwise, the five share one file: `AGENTS.md`, edited by `judge-surface-delegation-text` (§ Rules) and `citation-check-in-task-check` (the Commands block). The coordinator splits that file by section and checks it with `git merge-tree` before the second merge. `run.rs` is in the scope of five draft stories; only `run-record-and-story-evidence` takes it here.

## Left out

- `story:step-cannot-forge-its-validators`: `run.rs`; first unit of wave 10 (the operator role's priority, security).
- `story:case-scoped-step-guard`, `story:no-inspect-for-stateless-harness` with `story:no-model-step-for-approval` (one unit), `story:scan-before-push`: `run.rs` or `validator.rs`; later waves, `run.rs` split by function where `git merge-tree` is clean.
- The chain after this wave: `story:scorecard-reader`, `story:promotion`, `story:judge-guard`, `story:self-improvement-protocol`.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at eb54f3a |
| model budget | not stated; N = 2 |
| agent file | `AGENTS.md` read |
| gates policy for `scan-text` | the bot route's (`--policy` or `B10X_GATES_POLICY`) |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) for each code unit; `judge-surface-delegation-text` is documents and protocol text, verified by the coordinator and one adversary pass. Every merge into `wave/2026-10-18-w9` waits for its grant.

## The commits this wave makes

The opening store commit, each unit's commits, the merges into `wave/2026-10-18-w9` (each under a grant), the closing store commit, the merge into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: opened on the operator role's approval; the five stories moved to active.
