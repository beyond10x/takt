# Wave 18: the gate compares what its scripts import, a turn that needs its mainline, transcripts that charge no step

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/18` cut from
`main`, implementor sub-agents in the shared tree, one adversarial review and one full gate before
the close. Every unit is a judge surface; the merge into `main` is the owner's grant while no
delegation is on record.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:gate-compares-what-its-scripts-import`, `story:the-gate-compares-python-entry-points` | `gate.run` compares at the base every module a gate script imports from its own directory and every Python entry point it runs | rsi |
| B | `story:a-turn-does-not-open-when-the-mainline-tip-cannot-be-read` | a turn whose mainline tip cannot be read does not open | rsi |
| C | `story:transcripts-root-failures-name-it-and-charge-no-step`, `story:transcripts-under-a-python-dir-charge-no-step` | a transcripts root that cannot be made is named as such and charges no step; transcripts under a directory Python imports from charge no step | rsi |

## Store at the open

Filed: ten stories from the predicates of waves 16 and 17's reviews, and `epic:takt-on-loom-sdk`
with its draft stories (the owner's decision of 2026-10-20: takt becomes an application on
`b10x-loom-sdk`). The epic's stories follow the open `rsi` stories.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 3913af6a (wave 17 merged) |

## Log

- 2026-10-20: wave opened from `main` at 3913af6a.
- 2026-10-20: units committed: B 79425fa7, C 4a1958c3, A e87ced3f; ESS 6771b1f6; the 0.4.0 changelog section 1b6f2515 (doc-writer) and the version bump 11b882e7. `cargo test --workspace --no-fail-fast` at e87ced3f: 1414 passed, 0 failed.
- 2026-10-20: adversarial review at 11b882e7: needs-revision. `scripts/check-finding`: four predicated stories not_reproduced at 11b882e7, reproduced at 3913af6a. Fix round 1: 4020acd2 (a gate script at the root of the tree compares as `-m` does, a regression against `main`; a nested namespace package compares whole), d1c62415 (ESS). Filed for later: Python run through a shell is not compared (story:gate-compares-python-run-through-a-shell).
- 2026-10-20: `task check` at d1c62415: exit 0, 1416 passed, 0 failed. The five stories moved to implemented. Also filed at this close: story:check-finding-runs-one-at-a-time, story:trial-agent-reports-to-the-monitor and the three ADR 0101 stories.
