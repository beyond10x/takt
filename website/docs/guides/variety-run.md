---
title: Variety run
description: Run takt on a unit already merged in another public repository, with the unit's own tests held out.
---

# Variety run

A variety run is a takt run on work it was not tuned for, a unit already merged in another
public repository:

1. Clone the repository and reset the clone to the unit's parent commit.
2. Write an intent from the unit (its story, its outcome, its acceptance), leaving its test file
   out.
3. Run takt with `--forge none`: no pull request is opened, and the finish line is the stop at the
   merge approval.
4. Copy the unit's test file onto the case branch's tip and run the test command there: the
   held-out check.

```sh
takt run --repo . --intent "$(cat intent.md)" --forge none --harness claude \
  --model claude-opus-5-5 --hours 1 --test-cmd "cargo test --locked --workspace --quiet"
```

`headstart-trial-1` ran this command on a unit of the worktree repository. Pin the model so runs
compare.

Results of the first five (2026-10-18, a0b8576): [Runs](../self-improvement/runs.mdx). A variety
run is recorded in `scenario-runs.jsonl` with `scenario: null`
([fields](https://github.com/beyond10x/takt/blob/main/docs/metrics/README.md#scenario-runsjsonl)).

Sources: `docs/metrics/README.md` § `scenario-runs.jsonl` and § Scenarios 2 to 6 and the variety
runs, `docs/runs/2026-10-18-trial-1-worktree.md`.
