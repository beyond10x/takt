---
format: aep.planning-md/3
id: story:bench-in-scorecard
kind: story
status: draft
title: A pinned benchmark subset joins the scorecard's scenarios for promotion
summary: promotion is judged on benchmark tasks as well as the scratch scenarios
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
- depends_on: story:bench-task-protocol
scope:
- confidence: inferred
  path: crates/takt/src/scorecard.rs
revision: 6
---
# Story: bench-in-scorecard

## Outcome

The scorecard that decides a promotion includes a pinned benchmark subset beside the scratch-repository scenarios, so a candidate that regresses on a benchmark task is not promoted.

## Context

design:recursive-self-improvement § Scorecard. Its token rule and its flakiness tolerance are still open questions, and benchmark tasks make both sharper.

How a benchmark task enters the scorecard: as a takt case whose id is `scenario-<corpus>-<task>-trial-<n>`. `scenario_of` already groups such an id as a scenario (`crates/takt/src/scorecard.rs:206`). The runners pass the id with `--case-id` (story:harbor-agent-adapter, story:swe-bench-mini-runner). What is missing is the rank. A `bench.task/1` case completes with `completed`, and the scorecard ranks that the same as a stuck run.

The scorecard reads `events.jsonl` and nothing else (`crates/takt/src/scorecard.rs:1-5`). What enters it is therefore takt's own outcome on the task, never the held-out grader's verdict. Whether promotion should also read that verdict stays open, as the design's "What it decides later" says. Which pinned tasks join the promotion set is also decided later.

Promotion needs no acceptance of its own. `takt promote` judges on the same comparison and refuses with exit 3 when a scenario is worse (`crates/takt-cli/src/main.rs:96`). story:promotion is implemented.

The rank is a scorecard definition, which is the owner's (AGENTS.md § Roles). `outcome` takes its next version in `METRICS` (`crates/takt/src/scorecard.rs:39`), so a comparison against a scorecard saved under version 1 prints both figures. The diff touches a judge surface (AGENTS.md § Rules).

Nothing in the check is live: the fixture cases are written by the test.

## Expected before

On takt at main, holds; after the change, fails:

- On the fixture pair of the acceptance, `takt scorecard --against` exits 0.

## Scope

- `crates/takt/src/scorecard.rs` (inferred)

## Acceptance

On a fixture pair built as `crates/takt-cli/tests/scorecard_cli.rs:78-136` builds its repositories (five cases `scenario-tb2-fixture-trial-<n>` on each side, 500 tokens a case; the baseline's cases stopped `Completed (completed)`, the candidate's `NoProgress`), `takt scorecard --repo <candidate> --against <the baseline's saved scorecard>` exits 3 with scenario `tb2-fixture` marked `worse`, where at d17bc32 it exits 0 because `rank` puts every completion outcome except `accepted` at the rank of `NoProgress` (`crates/takt/src/scorecard.rs:215-222`).
