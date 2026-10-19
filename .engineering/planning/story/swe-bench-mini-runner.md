---
format: aep.planning-md/3
id: story:swe-bench-mini-runner
kind: story
status: draft
title: One SWE-bench Verified Mini instance becomes one graded prediction
summary: the issue text becomes the intent, the repository diff becomes the patch, the SWE-bench harness grades it
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-task-protocol
- depends_on: story:takt-in-container
- depends_on: story:bench-subset-pinned
- depends_on: story:takt-static-binary
scope:
- confidence: inferred
  path: docs/bench/swe-bench
revision: 8
---
# Story: swe-bench-mini-runner

## Outcome

takt turns one SWE-bench Verified Mini instance into a prediction the SWE-bench harness can grade. The issue text is the intent. The run works on the instance's repository at its base commit, under `bench.task/1`. The diff against the base commit, leaving out takt's own `.engineering/` records, becomes the `model_patch` of one line in a predictions file. Which instances run is story:bench-subset-pinned's subset, and grading that subset is the live round, not this story.

## Context

SWE-bench Verified Mini is a random 50-task subset of SWE-bench Verified (Hugging Face MariusHobbhahn/swe-bench-verified-mini, on the HAL leaderboard). The SWE-bench harness evaluates predictions in Docker environments (design:benchmark-integration). Check the predictions format (`instance_id`, `model_name_or_path`, `model_patch`) and the evaluation command against the official documentation when the story starts.

The previous outcome promised all 50 tasks while the acceptance named 10 unnamed ids. This story now delivers the per-instance runner. story:bench-subset-pinned names the ten instance ids and the dataset revision.

takt commits its case under `.engineering/cases/` on its own branch, so the patch is taken outside that directory. Otherwise the grader would apply takt's records as part of the fix.

SWE-bench exposes no test command per instance: its FAIL_TO_PASS and PASS_TO_PASS tests are the held-out acceptance. The runner passes the visible test command the subset file names for the instance, or `true` when it names none. It passes `--case-id scenario-swe-<instance>-trial-<n>` (story:bench-in-scorecard).

The live part is not in the acceptance: building each instance's environment, running a model and grading with the SWE-bench harness need Docker, the dataset and a model login. The verdict and the raw usage of that round become rows under story:bench-runs-schema.

## Expected before

On takt at main, holds; after the change, fails:

- No runner for SWE-bench tasks exists in this repository.

## Scope

- `docs/bench/swe-bench/run-takt.sh` (inferred)
- `docs/bench/swe-bench/fixtures/instance.json` (inferred)

## Acceptance

With a `claude` double first on PATH whose first step writes `work.txt`, `docs/bench/swe-bench/run-takt.sh docs/bench/swe-bench/fixtures/instance.json <out>` on the fixture instance (a local repository, its `base_commit` and a `problem_statement`) appends to `<out>/predictions.jsonl` one line whose `model_patch` is the one-file diff that adds `work.txt`, where at d17bc32 `docs/bench/swe-bench/run-takt.sh` does not exist.
