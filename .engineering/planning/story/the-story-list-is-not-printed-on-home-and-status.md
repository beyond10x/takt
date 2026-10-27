---
format: aep.planning-md/3
id: story:the-story-list-is-not-printed-on-home-and-status
kind: story
status: draft
title: Home and status show the open wave's stories, not all 170
summary: site re-screen s2-03
tags:
- docs
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-story-list-is-not-printed-on-home-and-status

## Outcome

The home page and the status page show the stories of the open wave and a count, with a link to
the full list, instead of printing all of them.

## Context

- Site re-screen of 2026-10-27, finding s2-03 (medium, confirmed): the full story list, about 170
  rows, is printed on the home page and again on the status page, making both pages over 30,000 px
  on a phone (390x844, also 768 and 1024); s1-07 (low) is the same at 1440 px, 13,776 px.

## Acceptance

1. `site/home-and-status-are-short`: `task site-check` at 390 px measures the home page and
   `/docs/status` each under 6,000 px, and both link the full list.

## Scope

Inferred: `website/scripts/generate.mjs` (the status), `website/product.json`, the status page.
Owner of the change: the doc-writer.
