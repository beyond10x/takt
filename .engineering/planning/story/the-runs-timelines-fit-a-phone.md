---
format: aep.planning-md/3
id: story:the-runs-timelines-fit-a-phone
kind: story
status: draft
title: The runs page's timeline charts fit their box at 390 px or say they scroll
summary: site re-screen s2-01
tags:
- docs
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-runs-timelines-fit-a-phone

## Outcome

On the self-improvement runs page at 390 px, each timeline chart fits its box or says it scrolls:
nothing is clipped on its right without a hint.

## Context

- Site re-screen of 2026-10-27, finding s2-01 (medium, confirmed): at 390x844 (also 768x1024 and
  1024x768) the timeline charts are 881 px wide inside a 358 px scroll box and clip their right
  side with no swipe hint, unlike the tables (`/docs/self-improvement/runs`).
- story:the-runs-charts-read-on-a-phone (implemented) put each chart's title and legend inside the
  viewport; the timelines' bodies are what is left.

## Acceptance

1. `site/runs-timelines-fit-or-hint`: `task site-check` at 390 px finds each timeline on
   `/docs/self-improvement/runs` no wider than its box, or with a visible scroll hint as the tables
   have.

## Scope

Inferred: `website/scripts/generate.mjs` (the charts), the runs page. Owner of the change: the
doc-writer.
