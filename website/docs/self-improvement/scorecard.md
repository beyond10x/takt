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

Per run of a case under `.takt/cases/` (or `.engineering/cases/`, where an earlier takt opened it):
its outcome, steps, tokens, refusals, `NoProgress`,
errors and wall time, read from the case's `events.jsonl`; a scenario run's outcome comes from the
held-out check's `held-out.txt` beside it. A case without a record is listed as `no record`.
Tokens are never read from a file the candidate could plant. The binary a case's runs were made
with is read from a `binary.txt` the operator wrote beside the record, kept as the operator's
statement, never as a figure.

## When a candidate is worse

`--against` compares each scenario with the same scenario in a saved scorecard, over cases.
Decided on 2026-10-18:

- **Trials**: the candidate runs n = 5 times per scenario; the baseline's runs are those saved with
  the promoted revision's scorecard, at least 5.
- **Outcomes**: ranked accepted, then `ApprovalRequired` or `Waiting`, then a budget stop, then
  `NoProgress` and every other stop, then an error. The candidate is worse at a rank when its cases
  at that rank or better fall short of the baseline's share by more than one case. At n = 5, one
  flaky case passes; two cases worse at any rank fail.
- **Tokens**: worse when the candidate's median exceeds the baseline's maximum. The rule tolerates
  the spread one unchanged harness shows on scenario 1 (106,003 to 167,249 tokens) and still
  catches a shift of the whole distribution.
- **Missing**: worse when the saved scorecard has a scenario this one has no run of.
- **No re-run**: a candidate that fails is not judged again until it changes, or until `--rejudge`
  brings a scorecard of new runs.

`takt scorecard --against` exits 3 when a scenario is worse.

## Promotion

`takt promote` checks the candidate's saved scorecard against the promoted baseline's under the
scorecard's rule. When no scenario is worse it builds a fresh clone of that revision confined
(`cargo build --locked --offline`, the environment cleared, no network): inside bubblewrap on
Linux, with only the clone, rustup's default toolchain and the registry cache visible; under
`sandbox-exec` on macOS, writing only its own directories. It installs the binary, records the promotion
under `~/.local/state/takt/promotions/`, where the latest record is the next baseline, and moves
the binary into place. Promotions run one at a time under the store's lock. `--install-root`
names where the binary goes; without it, `CARGO_INSTALL_ROOT`, else `CARGO_HOME`, else `~/.cargo`.

From 0.2.2, `takt promote` keeps the binary it replaces, executable, at
`$XDG_STATE_HOME/takt/judges/<sha256>`, and prints that path. An improvement turn that judged the
promotion finishes under that binary ([Running a turn](./the-loop.md#running-a-turn)). Kept
binaries are not removed.

On Linux the build runs under bubblewrap, on macOS under `/usr/bin/sandbox-exec` (since 0.10.0); [Getting started](../getting-started.md#what-it-needs) says what differs.

Sources: `takt scorecard --help`, `takt promote --help`, `crates/takt/src/scorecard.rs`,
`design:recursive-self-improvement` § Scorecard, `docs/metrics/README.md`.
