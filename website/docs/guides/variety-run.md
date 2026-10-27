---
title: Variety run
sidebar_position: 9
description: Run takt on a unit already merged in another public repository, with the unit's own tests held out.
lede: A variety run tests takt on work it was not tuned for, a unit someone already merged elsewhere, and judges it by the tests the unit shipped with.
source: takt run --help at 0.14.0; docs/metrics/README.md § scenario-runs.jsonl and § Scenarios 2 to 6 and the variety runs; docs/runs/2026-10-18-trial-1-worktree.md
---

# Variety run

A variety run is a takt run on work it was not tuned for: a unit already merged in another
public repository.

1. Clone the repository and reset the clone to the unit's parent commit.
2. Write an intent from the unit (its story, its outcome, its acceptance), leaving its test file
   out.
3. Run takt with `--forge none`: no pull request is opened, and the finish line is the stop at
   the merge approval.
4. Copy the unit's test file onto the case branch's tip and run the test command there. That is
   the held-out check.

```sh
takt run --repo . --intent "$(cat intent.md)" --forge none --harness claude \
  --model <model> --hours 1 --test-cmd "cargo test --locked --workspace --quiet"
```

The first variety run used this command on a unit of a public repository. Pin the model, so
that runs compare.

Results of the first five (2026-10-18, a0b8576): [Runs](/docs/self-improvement/runs). A variety
run is recorded in `scenario-runs.jsonl` with `scenario: null`
([fields](https://github.com/beyond10x/takt/blob/main/docs/metrics/README.md#scenario-runsjsonl)).
