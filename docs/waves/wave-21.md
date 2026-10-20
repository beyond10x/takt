# Wave 21: run tokens that are never a guess, takt pinned to Loom

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/21` cut from
`main`, implementor sub-agents in the shared tree, one adversarial review and one full gate before
the close. Every unit is a judge surface.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:run-tokens-unknown-is-not-zero` | a run whose harness step recorded no tokens has unknown run tokens; `takt scorecard --against` and `takt promote` refuse to judge tokens on such a set, naming the case and the step; the tokens metric's version moves | rsi |
| B | `story:loom-sdk-pinned-with-evaluation-parity` | takt depends on `b10x-loom-sdk` at tag 0.27.0 with one copy of Canon and engineering-protocols; Loom's governor evaluates the 30 recorded decisions as takt did, as parsed values; the granted ones differ only by the missing grant | loom |

## Order

A first (owner's delegate: no promotion until run tokens are never 0 by default), B beside it: the
two units touch disjoint files.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 81f34956 (wave 20 merged, 0.5.0) |
| Loom | tag 0.27.0 at 3e2cbbbf |

## Log

- 2026-10-20: wave opened from `main` at 81f34956.
- 2026-10-20: units committed: A f13dee7b, B a6a2d337; ESS caf5ed8. Adversarial review at caf5ed8: needs-revision; fix round 1: 3cdf8c6 (a 0.5.0-shaped copy reads again, the refusal only under the tokens definition 2; the parity check pins the granted actions' reasons and explanation). The real store's latest baseline has no untokened harness step (read-only).
- 2026-10-20: `task check` at 3cdf8c6: exit 0, 1496 passed, 0 failed, 17 ignored. The two stories moved to implemented. No wave 22 opens until the owner is back (account budget).
