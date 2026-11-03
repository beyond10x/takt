---
format: aep.planning-md/3
id: approval-record:wave-50-four-stories
kind: approval-record
status: approved
title: 'Wave 50: the forge write outcome, the dollar budget, the unrelated install and the image'
summary: one wave of four units for 0.29.0; the image story is built and released already
relations:
- decides: story:fenced-forge-dispatch
- decides: story:bench-subset-and-budget
- decides: story:an-unrelated-program-install-refuses-no-step
- decides: story:release-publishes-the-image
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-03T17:25:26Z", actor: "human:timo", revision: 2}
---
# Approval: wave 50

## Decision

The owner, 2026-11-03, through the orchestrator: one takt wave of four units for the next minor,
0.29.0, over the active stories story:fenced-forge-dispatch, story:bench-subset-and-budget,
story:an-unrelated-program-install-refuses-no-step and story:release-publishes-the-image. The
release prepare is a commit of the wave. A draft from a finding does not enter.

story:release-publishes-the-image entered as the fourth unit on the owner's word of 2026-11-02
12:54Z, relayed verbatim: "Yes, tagged and the chart and loom can be public. Yeah, so option A."
Here "tagged" is takt, and option A is the takt image published publicly on
`ghcr.io/beyond10x/takt`. Its bounds: the workflow's `GITHUB_TOKEN` with `packages: write`, no new
secret, no new credential, no spending; the image publishes only from the release tag's workflow
run, and the wave pushes no image by hand. approval-record:container-image-published-on-release
holds the earlier approval of the same story.

The expected-before probe of story:an-unrelated-program-install-refuses-no-step copies files into
the program directory of the real home. It is run only against a home of its own under the unit's
scratch root, never against the machine's.

## Selection, read at origin/main 40843ed9

`aep plan artifact waves --kind story --status active` (aep 0.79.0), after the wave's typed scope
entries were written, verbatim:

```
wave 1
  story:an-unrelated-program-install-refuses-no-step (inferred)
  story:fenced-forge-dispatch (inferred)
  story:harness-section-is-metaharness-session-request
  story:release-publishes-the-image
wave 2
  story:bench-subset-and-budget (inferred)
wave 3
  story:harness-flags-spell-metaharness-names (inferred)
collision: story:an-unrelated-program-install-refuses-no-step story:bench-subset-and-budget crates/takt/src/run.rs (inferred)
collision: story:bench-subset-and-budget story:harness-flags-spell-metaharness-names crates/takt-cli/src/main.rs (inferred)
collision: story:bench-subset-and-budget story:harness-section-is-metaharness-session-request crates/takt-cli/src/main.rs (inferred)
collision: story:bench-subset-and-budget story:harness-section-is-metaharness-session-request crates/takt/src/harness.rs (inferred)
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt-cli/src/main.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request ess/ess-inputs.yaml
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request website/docs/reference/configuration.md
unassessed: story:harness-child-confined
3 wave(s), 7 collision(s), 1 unassessed
```

- The two harness stories and story:harness-child-confined are not in the wave (blocked upstream
  and blocked: dependency).
- story:bench-subset-and-budget and story:an-unrelated-program-install-refuses-no-step collide on
  `crates/takt/src/run.rs`: the budget unit starts from `wave/50` after the guard unit has merged.
- story:fenced-forge-dispatch: its four `depends_on` stories are `implemented`, among them
  story:an-unattended-run-merges-under-a-delegated-grant (moved 2026-10-30T00:45:25Z on a CI test
  result; built in 4def3240a), whose only blocker decision-blocker:unattended-merge-grant is
  `cleared` (2026-10-29T21:29:55Z). `aep plan artifact blocked` and `list` show no block on it.
- story:release-publishes-the-image: built in 1d1e4554c (2026-11-02), first released in 0.27.0.
  The release runs of 0.27.0 (41432901115) and 0.28.0 (41667833256) each passed the job
  `Publish the image / takt / image (publish)`, and the registry holds `0.27.0` and `0.28.0`. The
  wave checks the acceptance against the tree and records it; no code.

## Commits this approval authorises

One commit or more per unit on its unit branch, the merges into `wave/50`, the store commits (this
record, scope, evidence, review results, moves), the release prepare for 0.29.0, one pull request
through the bot route, and its merge to `main` on green CI. Tag and release follow the
orchestrator's go.
