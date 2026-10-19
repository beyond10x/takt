---
title: Scorecard
description: How takt compares a candidate revision with the promoted baseline, and when it promotes one.
---

# Scorecard

```sh
takt scorecard --repo <scenario repo> --save <dir> --revision <takt sha>
takt scorecard --repo <scenario repo> --against <dir>/scorecard-<baseline sha>.json
takt promote --repo <takt checkout> --scorecard <dir>/scorecard-<candidate sha>.json --dry-run
```

## What it reads

Per run of a case under `.engineering/cases/`: its outcome, steps, tokens, refusals, `NoProgress`,
errors and wall time, read from the case's `events.jsonl` only. A case without one is listed as
`no record`. Tokens are never read from a file the candidate could plant. The binary a case's runs
were made with is read from a `binary.txt` the operator wrote beside the record, kept as the
operator's statement, never as a figure.

## When a candidate is worse

`--against` compares each scenario with the same scenario in a saved scorecard, over cases. The
rule it applies is printed first. Decided on 2026-10-18 by the planner, under the owner's standing
grant:

- **Trials**: the candidate runs n = 5 times per scenario; the baseline's runs are those saved with
  the promoted revision's scorecard, at least 5.
- **Outcomes**: ranked accepted, then `ApprovalRequired` or `Waiting`, then a budget stop, then
  `NoProgress` and every other stop, then an error. The candidate is worse at a rank when its count
  of cases at that rank or better is more than one below the baseline's. At n = 5, one flaky case
  passes; two cases worse at any rank fail.
- **Tokens**: worse when the candidate's median exceeds the baseline's maximum. The rule tolerates
  the spread one unchanged harness shows on scenario 1 (106,003 to 167,249 tokens) and still
  catches a shift of the whole distribution.
- **Missing**: worse when the saved scorecard has a scenario this one has no run of.
- **No re-run**: a candidate that fails is not judged again until it changes.

`takt scorecard --against` exits 3 when a scenario is worse.

## Promotion

`takt promote` checks the candidate's saved scorecard against the promoted baseline's under the
scorecard's rule. When no scenario is worse it builds a fresh clone of that revision inside
bubblewrap (`cargo build --locked --offline`, the environment cleared, only the clone, rustup's
default toolchain and the registry cache visible), installs the binary, records the promotion
under `~/.local/state/takt/promotions/`, where the latest record is the next baseline, and moves
the binary into place. One promotion a UTC day, one at a time under the store's lock.

bubblewrap is Linux only, so on macOS a promotion refuses with exit 1 before it judges;
`--dry-run` and `--baseline-only`, which build nothing, still run.

## Against the first turn's figures

Turn 2's candidate 98719c3 had a median of 122,465 tokens over 5 runs; the baseline a0b8576's
maximum was 138,160. Under the token rule above it is not worse. This is a reading of the rows in
`scenario-runs.jsonl`, not a scorecard record: `improvement-turns.jsonl` holds no line for turn 2.

Sources: `takt scorecard --help`, `takt promote --help`, `design:recursive-self-improvement`
§ Scorecard, `docs/metrics/README.md`.
