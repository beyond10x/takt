---
format: aep.planning-md/3
id: story:the-scorecard-output-block-keeps-its-columns
kind: story
status: draft
title: The scorecard output block on Read a case keeps its columns
summary: site re-screen s1-01
tags:
- docs
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-scorecard-output-block-keeps-its-columns

## Outcome

On the site's "Read a case" guide, the `takt scorecard` output block keeps one table row per line
at 1440 px, its columns aligned; a wider block scrolls sideways instead of wrapping.

## Context

- Site re-screen of 2026-10-27, finding s1-01 (medium, confirmed): at 1440x900 the scorecard
  output in a code block wraps, so the table's columns no longer line up and every row splits in
  two (`/docs/guides/read-a-case`).

## Acceptance

1. `site/scorecard-block-does-not-wrap`: `task site-check` at 1440 and 390 px finds no code block
   on `/docs/guides/read-a-case` whose rendered lines outnumber its source lines.

## Scope

Inferred: `website/docs/guides/read-a-case.md`, the site's code-block style. Owner of the change:
the doc-writer.
