---
title: Scenario run
description: Replay a declared scenario from a fresh trial branch, so runs of different revisions can be compared.
---

# Scenario run

A scenario is a story of a scratch repository at a known seed commit. Replaying it from the same
seed is how takt's revisions are compared with each other and with a harness running free.

```sh
takt scenario run --repo <path> <id> --hours 1
```

`.engineering/scenarios.conf` on `main` names each scenario's story and seed commit. The run:

1. checks that the tree is clean;
2. fetches the remote's `trial/*` branches through the bot route;
3. cuts `trial/<n>` from the seed, `<n>` one past the highest the checkout then knows;
4. refuses unless the store there shows the story ready;
5. pushes the branch with a lease that only lets it create it, and refuses unless the push
   reports it created;
6. opens the case `scenario-<id>-trial-<n>` on `story.delivery/1`, whose pull request targets
   `trial/<n>`.

Nothing is reset by a force-push, and nothing of an earlier run is resumed. A refusal before the
push removes the branch and puts the tree back.

A trial branch is deleted afterwards with
`b10x-gates bot --repository <owner>/<repo> -- delete-branch trial/<n>`, which the bot route
admits only while the branch's tip is merged into the default branch.

## The scenarios measured so far

Scenario 1 is the scratch repository's `story:remove-a-todo`: a 31-line change to a toy CLI.
Scenarios 2 to 6 were added on 2026-10-18 and each run once. The held-out check of each is a
script no agent saw. Results: [Runs](../self-improvement/runs.mdx).

Sources: `takt scenario run --help`, `design:recursive-self-improvement` § Scenario corpus,
`docs/metrics/README.md`.
