---
format: aep.planning-md/3
id: epic:continuous-orchestration
kind: epic
status: draft
title: 'Continuous orchestration: a repository''s delivery as a tree of cases under one dispatcher'
summary: build steps 2-8 of design:continuous-orchestration; decomposed before wave 14
relations:
- derived_from: design:continuous-orchestration
- serves: vision:safe-agentic-coding
revision: 2
---
# Epic: continuous-orchestration

## Outcome

One repository's whole delivery runs as a tree of Canon cases under one deterministic dispatcher: planning is a unit (`plan.refill/1`), a wave is a case with child cases, a run for a bounded time keeps a low-water mark of ready stories so its workers never run out of work, and a run can be paused and steered without losing work (product-requirements:takt R16–R18).

## Context

design:continuous-orchestration lays out eight build steps (§ 11): (1) story:declared-loops; (2) the root case, `case.dispatch`, the children artifact and the Workspace seam; (3) the dispatcher as the single store writer, lock reclaim, cumulative tokens and pick records; (4) `wave.delivery/1`; (5) roles, the write allow-list and `review.panel/1`; (6) `plan.refill/1`; (7) pause and the new stops (exit 7 Paused, 8 FrontierExhausted); (8) across repositories. Only step 1 has a story. This epic holds steps 2–8; each becomes one or more stories when the release plan's standing rule needs them, decomposed by the planner with the plan-critic panel, before wave 14.

## Acceptance

A `takt run --hours 1` on a repository with a planning store, two ready stories and an empty buffer keeps working the whole hour: it plans further stories when the buffer falls below its mark, delivers a wave with parallel children, and a pause written mid-run ends with every open case resumable from its files.
