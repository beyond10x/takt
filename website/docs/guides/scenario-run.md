---
title: Scenario run
sidebar_position: 8
description: Replay a declared scenario from a fresh trial branch, so runs of different takt revisions can be compared on the same work.
lede: A scenario is a story at a known seed commit, and replaying it from that seed on a fresh branch is how one takt revision is compared with another.
source: takt scenario run --help at 0.14.0; docs/metrics/README.md; the design record of recursive self-improvement, § Scenario corpus
---

# Scenario run

A scenario is a story of a scratch repository at a known seed commit. Replaying it from the same
seed is how takt's revisions are compared with each other, and with a harness running without
takt.

```sh
takt scenario run --repo <path> <id> --hours 1
```

`.engineering/scenarios.conf` on `main` names each scenario's story and seed commit.
`--declared-on` reads it from another revision. The run:

1. checks that the tree is clean;
2. fetches the remote's `trial/*` branches through the bot route;
3. cuts `trial/<n>` from the seed and checks it out, `<n>` one past the highest the checkout
   knows, local or remote;
4. refuses unless the store there shows the story ready;
5. pushes the branch with a lease that only lets it create it, and refuses unless the push
   reports it created;
6. opens the case `scenario-<id>-trial-<n>` on `story.delivery/1`, whose pull request targets
   `trial/<n>`.

Nothing of an earlier run is resumed. A refusal before the push removes the branch and puts the
tree back. Without the bot route nothing is fetched or pushed, and `<n>` is 22 digits instead:
the UTC time to the second and a random 8-digit nonce. A scenario run refuses `--forge gitlab`.

A trial branch is deleted afterwards with
`b10x-gates bot --repository <owner>/<repo> -- delete-branch trial/<n>`, which the bot route
admits only while the branch's tip is merged into the default branch.

`takt scenario run` takes the same budget, harness, endpoint, forge and test flags as
`takt run`, `--continue-past-red-base` among them.

`takt scenario run --config <FILE>` reads the same run configuration file as `takt run`, and
refuses a `story` in it, since the scenario names the story. (Not in 0.14.0.)

## Scenarios

Scenario 1 is a 31-line change to a toy CLI in the scratch repository. Scenarios 2 to 6 were
added on 2026-10-18 and each run once. The held-out check of each is a script no agent saw.
Results: [Runs](/docs/self-improvement/runs). `takt scorecard` reads a scenario run's outcome from
the held-out check beside its record ([Read a case](./read-a-case.md#every-run-of-the-repository-at-once)).
