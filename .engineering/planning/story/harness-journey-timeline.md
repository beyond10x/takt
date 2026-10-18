---
format: aep.planning-md/3
id: story:harness-journey-timeline
kind: story
status: implemented
title: All harness runs of a scenario overlaid on one timeline in one vocabulary
summary: lanes per run on a shared time axis, operations normalised across takt, Claude Code and Codex, so waste is visible as shape
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:metrics-over-time
scope:
- confidence: inferred
  path: docs/metrics/journey.py
- confidence: inferred
  path: docs/metrics/journey.svg
- confidence: inferred
  path: docs/metrics/journeys.jsonl
- confidence: inferred
  path: docs/pitch/index.html
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T16:37:27Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T16:37:27Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-18T16:37:27Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: harness-journey-timeline

## Outcome

Every recorded run of a scenario, under takt or under a harness running free, is drawn on one timeline: one lane per run, a shared time axis from each run's start, and every operation as a mark in one vocabulary (model call, file read, file edit, test run, gate, push, pull request, CI wait, approval wait, refusal, resume). Runs of different harnesses and revisions overlay, so waste shows as shape: CI run twice, a step that changes nothing before a stop, a long wait on the forge, a model call that only asks for an approval.

## Context

Owner request of 2026-10-18: "using the idea of this context-lense, normalizing of agent journey, i would like to see all harness runs overlayed on a timeline so it can easily be visualized and potential improvements detected visually". It is the first view of story:normalized-harness-trace, which keeps one operation vocabulary across harnesses and keeps raw usage for every token and cost figure.

What the scenario 1 runs of 2026-10-18 (docs/metrics/scenario-runs.jsonl) can and cannot give:
- takt runs: one line per step in the run log, in order, with no time; since story:run-record-and-story-evidence, `events.jsonl` timestamps at 1 s resolution; per model call, the claude JSON a shim kept (end time, turns, usage).
- free Claude Code runs: one final JSON per run (`num_turns`, `duration_ms`, usage), with no per-call times, because they ran with `--no-session-persistence` and `--output-format json`.
- Codex runs: `--json` events in order (`item.started` / `item.completed`), without timestamps per item as recorded, and `--ephemeral` kept no rollout.

So the existing runs give an ordered journey per run and a total time, and a time axis only for takt's own events; later runs capture the stream (`claude -p --output-format stream-json --verbose`, Codex `--json` with arrival times taken by the runner) so every lane gets real times.

The overlay already shows today's two known overheads, story:pr-opens-when-change-is-done (pr.open and ci.read at the seed, CI twice) and story:no-inspect-for-stateless-harness (an inspect step, then NoProgress), and story:no-model-step-for-approval (a model call whose only effect is an approval request).

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/metrics/journey.svg` exits non-zero.

## Scope

- `docs/metrics/journey.py`: reads each run's raw records named in `scenario-runs.jsonl`, normalises them into journey events in the shared vocabulary, writes `docs/metrics/journeys.jsonl` and draws `docs/metrics/journey.svg` (standard library only) (inferred)
- `docs/pitch/index.html`, the public site: embed the overlay (inferred)
- the baseline runner (outside the repository): capture streamed events with arrival times for the free arms (inferred)

## Acceptance

`python3 -I docs/metrics/journey.py` draws one lane per recorded scenario 1 run, grouped by arm and revision, with each operation in the shared vocabulary and its source record named; a lane whose source has no times is drawn as ordered steps and labelled "order only"; on takt's lanes the seed-time pr.open, the second CI and the inspect-then-NoProgress steps are visible; rerunning after one new run adds one lane with no hand edit; no token or cost figure on the view is taken from the normalised events.
