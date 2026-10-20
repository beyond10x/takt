# Wave 17: an outcome metric that names its stops, a turn that refuses a merged candidate, the dispatcher and trial profiles, a run viewer

Stage: **closed**. Shape: one checkout, one wave branch `wave/17` cut from `main`, implementor
sub-agents in the shared tree, units committed by the dispatcher, one adversarial review and one
full gate before the close. The wave's merge into `main` is granted by the owner: two units add
agent roles and widen one role's authority.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:outcome-metric-takes-version-3` | `METRICS` gives `("outcome", 3)`; a takt that knows version 2 refuses a version 3 scorecard; a version 2 baseline meets a version 3 candidate only after `--rebaseline` | rsi |
| B | `story:a-turn-refuses-a-candidate-main-already-holds` | a turn refuses at open a candidate the mainline already holds, naming both ids and the way past | rsi |
| C | `story:disk-low-exits-with-its-own-code` | `DiskLow` exits 8 for `takt run` and `takt scenario run`; the help says so | rsi |
| D | `story:one-session-dispatches-the-waves`, `story:one-session-runs-the-trials` | the dispatcher's and the trial agent's profiles; the agent text no longer describes two sessions, per-story worktrees or a message contract | process |
| E | `story:takt-view-renders-runs` | `takt view <dir>` writes one offline HTML file, one lane per run, a click on a mark opens the step's transcript | process |

## Order

A, B and C first (three implementors). C and E both touch `crates/takt-cli/src/main.rs`: E's
subcommand is wired after C's commit. D after A, B and C return.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 37e7c71d (wave 16 merged) |

## Log

- 2026-10-20: wave opened from `main` at 37e7c71d.
- 2026-10-20: check.yml on `main` 37e7c71d failed three `commit_identity_cli` tests (gate.run refused for `~/.local` on a runner whose home had none); first unit 2802f440 seals the tests' state directory. Units committed: C a7f0338d, B 64555559, A 98ccdcf2, D 3c068303, E 5877c265 (+ cae76cb8), ESS ef8609da.
- 2026-10-20: `cargo build --workspace` exit 0; `cargo test --workspace --no-fail-fast` at cae76cb8: 1396 passed, 0 failed.
- 2026-10-20: adversarial review at ef8609da: needs-revision. `scripts/check-finding`: A and C reproduced at 37e7c71d, not_reproduced at ef8609da. Fix round 1: 33dd0398 (the transcripts directory is made when the run opens; the defect 2802f440 only hid in the tests), 91acce18 (the profiles: the close grants the sha it merges, one owner of `takt promote`, five trials per side, `check-finding` before hand-over), 44c4f7a7 (ESS).
- 2026-10-20: `task check` at 44c4f7a7: exit 0, 1397 passed, 0 failed. The six stories moved to implemented. The wave changes agent authority (one dispatcher grants its own waves and tags releases; the trial agent runs `takt promote`), so its merge into `main` is the owner's grant.
- 2026-10-20: the reviewer's confirm at 00a900ac: approve. The owner granted the merge and confirmed waves 15 and 16; merged into `main` as 3913af6a.
