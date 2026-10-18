# Wave 06: Fix every finding of the 2026-10-18 code review and live trials

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running** (batch A dispatched). Approval: the operator approved option A on 2026-10-18 (one wave, two batches, function split in batch A), including proceeding with the one uncommitted file in the main checkout. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:tree-hygiene` | `vision:safe-agentic-coding` | open refused on a dirty tree; a step commits exactly its paths; a refusal discards index, ignored files, `.git/` writes and directory swaps; porcelain parsed with `-z`; a take is committed; `open_next` reads the base | `git.rs`, `run.rs` (step section, `restore`/`snapshot`, `open_next`), `case.rs` (open/create), `harness.rs` (`is_test_path`) (cited) |
| `story:validator-confinement` | `vision:safe-agentic-coding` | a test command that writes a grant, a record or a gates file is refused and reverted; gates and project file read from the base revision; child env without tokens, timeout, bounded output; no project/user hooks in the harness child; `gh --repo`; `CiRead` mutant killed | `validator.rs` (but `PrMerged`), `harness.rs` (spawn/env), `forge.rs`, `run.rs` (validator section), `frontier.rs` (`aep` spawn) (cited) |
| `story:authority-hardening` | `vision:safe-agentic-coding` | lock by `create_new`, 300/300; grant bound to a revision and only at the gate; `approve` records first; case ids sanitised; unreadable record stops the run; `latest_decision` numeric | `case.rs` (lock, authority, resume, decisions, evidence), `frontier.rs` (`approve`), `validator.rs` (`PrMerged`), `run.rs` (`is_due`) (cited) |
| `story:cli-protocol-defaults` | `vision:safe-agentic-coding` | store story defaults to `story.delivery/1` (embedded); `--protocol` mismatch on resume refused; forge refusal is a `stopped:` line; `ApprovalRequired` exit code differs from `Completed`; `--forge` help true | `takt-cli/src/main.rs` (cited), `run.rs` (`resume`, error-to-stop) (inferred), `protocol.rs` (inferred) |
| `story:loop-test-gaps` | `vision:safe-agentic-coding` | mutants 17 A–D each turn a test red; "nothing ready twice" asserted through `open_next`; `Frontier::next` used or gone | `tests/loop_gaps.rs` (new), `tests/frontier.rs` (cited), `frontier.rs` (inferred) |
| `story:docs-and-spec-true` | `vision:safe-agentic-coding` | `ess specify validate` exit 0; every `read from:` citation resolves; design status, README, AGENTS.md true | `ess/domains/takt.yaml`, `docs/design/takt.md`, `README.md`, `AGENTS.md` (cited) |

`story:docs-and-spec-true` `depends_on` the other five.

## Selection path

`aep plan artifact waves --kind story --status draft --format text` (aep 0.74.0). The verb places the six stories in five waves (1: authority-hardening; 2: cli-protocol-defaults, loop-test-gaps; 4: tree-hygiene; 5: validator-confinement; 6: docs-and-spec-true), because the typed scope is per file and the units share `run.rs`, `case.rs`, `frontier.rs`, `validator.rs` and `harness.rs`.

**Disagreement with the verb, reported:** the operator asked for one wave on one fix branch. This page runs it as one wave in two batches, using the split-one-file-by-function fallback for batch A: each brief names the functions it owns in each shared file, each implementor pastes its hunk headers, and `git merge-tree --write-tree` runs on every pair before the first merge. The verb's file-level reading is correct; the split rests on function ranges the typed scope cannot express.

| batch | units | runs |
|---|---|---|
| A | tree-hygiene, validator-confinement, authority-hardening, cli-protocol-defaults | in parallel, N = 4 |
| B1 | loop-test-gaps | after A has merged (line numbers in `run.rs`, `frontier.rs` move) |
| B2 | docs-and-spec-true | after B1 has merged (citations are checked against the final code) |

## The verb's three lists, verbatim (wave-06 stories only; full output 11 waves, 92 collisions, 0 unassessed)

### Waves

- wave 1: `story:authority-hardening (inferred)`
- wave 2: `story:cli-protocol-defaults (inferred)`, `story:loop-test-gaps (inferred)`
- wave 4: `story:tree-hygiene (inferred)`
- wave 5: `story:validator-confinement (inferred)`
- wave 6: `story:docs-and-spec-true`

### Collisions between wave-06 stories (12)

- collision: story:authority-hardening story:cli-protocol-defaults crates/takt/src/run.rs (inferred)
- collision: story:authority-hardening story:loop-test-gaps crates/takt/src/frontier.rs (inferred)
- collision: story:authority-hardening story:tree-hygiene crates/takt/src/case.rs
- collision: story:authority-hardening story:tree-hygiene crates/takt/src/run.rs
- collision: story:authority-hardening story:validator-confinement crates/takt/src/frontier.rs
- collision: story:authority-hardening story:validator-confinement crates/takt/src/run.rs
- collision: story:authority-hardening story:validator-confinement crates/takt/src/validator.rs
- collision: story:cli-protocol-defaults story:tree-hygiene crates/takt/src/run.rs (inferred)
- collision: story:cli-protocol-defaults story:validator-confinement crates/takt/src/run.rs (inferred)
- collision: story:loop-test-gaps story:validator-confinement crates/takt/src/frontier.rs (inferred)
- collision: story:tree-hygiene story:validator-confinement crates/takt/src/harness.rs
- collision: story:tree-hygiene story:validator-confinement crates/takt/src/run.rs

### Unassessed (0)

- none

## Left out

- Finding 30, public module surface (`lib.rs` all `pub`): a visibility change ripples into every test target; recorded as not fixed in this wave. Its exit-code half is in `story:cli-protocol-defaults`.
- Grant revocation (`approve` cannot deny or revoke): a feature, not a defect; not in this wave.
- Handoff findings 2 (validator-owned actions offered when no validator is registered), 3 (kernel-side stop at the gate), 5 (run writes nothing to the story), 6 (cost compared to nothing): open stories already hold them (`story:run-record-and-story-evidence`, `story:story-done-end-to-end`).

## Pre-flight

| check | value |
|---|---|
| free disk | 145G on `/` |
| tree | main checkout on `main` at 01b72c6 with one uncommitted file, `.engineering/planning/decision-blocker/live-trial-repository.md`, another session's; no unit's surface touches it; the integration tree is forked from 01b72c6 and does not carry it |
| worktrees | wave 05's trees finished; the `pr-open-needs-commit` tree finished and archived |
| build | each tree its own `target/` (a finished tree's cache was 1432 MiB); build time not measured this wave, wave 05's page: incremental under 1 min, `task check` 2-3 min |
| model budget | not stated by the operator; N = 4 (the skill's default) |
| agent file | `AGENTS.md` read |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `fix/2026-10-18-review` | managed tree `wave-06-integration` | its `target/` | `~/.cache/takt-wave-06/scratch/int` |
| tree-hygiene | `impl/tree-hygiene` | managed tree `w6-tree-hygiene` (planned) | its `target/` | `~/.cache/takt-wave-06/scratch/tree-hygiene` |
| validator-confinement | `impl/validator-confinement` | managed tree `w6-validator-confinement` (planned) | its `target/` | `~/.cache/takt-wave-06/scratch/validator-confinement` |
| authority-hardening | `impl/authority-hardening` | managed tree `w6-authority-hardening` (planned) | its `target/` | `~/.cache/takt-wave-06/scratch/authority-hardening` |
| cli-protocol-defaults | `impl/cli-protocol-defaults` | managed tree `w6-cli-protocol-defaults` (planned) | its `target/` | `~/.cache/takt-wave-06/scratch/cli-protocol-defaults` |
| loop-test-gaps | `impl/loop-test-gaps` | managed tree `w6-loop-test-gaps` (planned) | its `target/` | `~/.cache/takt-wave-06/scratch/loop-test-gaps` |
| docs-and-spec-true | `impl/docs-and-spec-true` | managed tree `w6-docs-and-spec-true` (planned) | its `target/` | `~/.cache/takt-wave-06/scratch/docs-and-spec-true` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) for the four code units of batch A; `aep:implementor` and one `aep:adversary` pass for `loop-test-gaps`; `aep:implementor` alone for `docs-and-spec-true`, verified by the coordinator against `ess specify validate` and a citation-resolving script. Unit brief at `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit (this page, the six stories, their move to `active`), each unit's commits, the merges into `fix/2026-10-18-review`, the closing store commit, the merge of `fix/2026-10-18-review` into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: proposed; approved (option A); the six stories moved to active.
