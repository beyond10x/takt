---
title: Variety run
description: Run takt on a unit already merged in another public repository, with the unit's own tests held out.
---

# Variety run

A variety run checks takt on real work it was not tuned for. It is a takt run on a unit already
merged in another public repository:

1. Clone the repository and reset the clone to the unit's parent commit.
2. Write an intent from the unit (its story, its outcome, its acceptance), leaving its test file
   out.
3. Run takt with `--forge none`: no pull request is opened, and the finish line is the stop at the
   merge approval.
4. Copy the unit's test file onto the case branch's tip and run the test command there. That is
   the held-out check.

```sh
takt run --repo . --intent "$(cat intent.md)" --forge none --harness claude \
  --model claude-opus-5-5 --hours 1 --test-cmd "cargo test --locked --workspace --quiet"
```

This is the command of `headstart-trial-1`, on a unit of the worktree repository; pin the model,
as it did, so runs compare.

## What the first runs showed

On 2026-10-18 five variety runs ran at a0b8576 on units of three repositories, n = 1 each. Four
passed the held-out check; `canon-1` failed 31 of 32, because the intent left out a rule the
failing test checks. They ran with no `--model` and kept no usage per call, so their records have
no cost and no model. The full table and its notes are on [Runs](../self-improvement/runs.mdx).

A variety run is recorded in `docs/metrics/scenario-runs.jsonl` with `scenario: null`, and
`repository`, `unit` and `seed` saying what it ran on.

Sources: `docs/metrics/README.md` § `scenario-runs.jsonl` and § Scenarios 2 to 6 and the variety
runs, `docs/runs/2026-10-18-trial-1-worktree.md`.
