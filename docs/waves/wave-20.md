# Wave 20: takt's gate readable to itself, trials that stop early, a judge on targeted steps

Stage: **closed**. Shape: one dispatcher in one checkout, one wave branch `wave/20` cut from
`main`, implementor sub-agents in the shared tree, one adversarial review and one full gate before
the close. Every unit is a judge surface. The close carries release 0.5.0.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:gate-exec-is-readable-by-the-gate` | takt's gate document binds commands the reader reads with certainty; no takt gate line fails closed | rsi |
| B | `story:a-re-trial-stops-when-its-verdict-is-decided` | `--active-seconds` budget excluding forge waits; the trial agent's stop rules, token cap, early stop and pre-flight | rsi |
| C | `story:the-judge-compares-the-steps-a-candidate-targets` | a per-step comparison with the whole run kept not worse; the middle-three rule before a re-trial | rsi |
| D | `story:a-step-replays-from-its-recorded-state` | `takt replay` runs one step of a recorded case from its recorded state | rsi |
| E | `story:a-run-refuses-a-scanned-identity-at-open` | an identity the forge's scan refuses ends the run before step 1 | rsi |

## Order

A, B and C first. B, D and E all touch `crates/takt-cli/src/main.rs`; D and E go after B's
commit. B holds every `.agents/trial-agent.md` change of the wave, C's parking rule included.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 78205d97 (wave 19 merged) |

## Log

- 2026-10-20: wave opened from `main` at 78205d97.
- 2026-10-20: units committed: A c3c4cbf2, B 35a4dc2e, C 59d93472, D 1cb37417, E 8a1d3c33; the fix for `main`'s red CI 5bb6993e (also applied to `main`); the 0.5.0 CHANGELOG section 6a8e2152, ESS 00bec511, the bump d7d4989d. `cargo test --workspace --no-fail-fast` at 8a1d3c33: 1469 passed, 0 failed, 17 ignored.
- 2026-10-20: adversarial review at d7d4989d: needs-revision; `task check` there exit 0. Fix round 1: a938b960 (`takt replay` never follows a link into a case; `--out` outside the case's tree), 404de157 (the identity-scan call order asserted; `ActiveTime` named at rank 1, the outcome metric staying at version 3). Fix round 2, on the owner's delegate's call: a89a4ece (a step that recorded no tokens is unknown to the step judge, never 0); the same defect at run level is a 0.5.0 known limit and wave 21's first unit.
- 2026-10-20: `task check` at a89a4ece: exit 0, 1478 passed, 0 failed, 17 ignored. The five stories moved to implemented.
- 2026-10-20: the confirm at 3ed68989: needs-revision, one medium in unit D (a relative `takt replay --out` resolved inside the repository by git). Fix round 3, granted once by the owner's delegate because it was a one-line fix of a medium finding: c6f9172a. `task check` at c6f9172a: exit 0, 1479 passed, 0 failed, 17 ignored.
- 2026-10-20: the confirm scoped to unit D at e0bd3ca2: approve; granted under the owner's standing exception; merged into `main` as 81f34956 (`task check` on the merge: exit 0, 1479 passed); check.yml green; tagged and released 0.5.0.
