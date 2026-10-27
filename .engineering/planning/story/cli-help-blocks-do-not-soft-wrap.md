---
format: aep.planning-md/3
id: story:cli-help-blocks-do-not-soft-wrap
kind: story
status: draft
title: CLI reference help blocks keep clap's line breaks at every width
summary: site re-screen s2-02
tags:
- docs
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: cli-help-blocks-do-not-soft-wrap

## Outcome

On the CLI reference page, each help block keeps clap's own line breaks at every width: option
names and their descriptions do not run together on a phone, and no stub lines are left at 1024 px.

## Context

- Site re-screen of 2026-10-27, finding s2-02 (medium, confirmed): at 390x844 and 1024x768 the CLI
  help blocks are soft-wrapped (`/docs/reference/cli`).
- The page is generated (story:cli-reference-is-generated-by-a-rust-crate moves its generator).

## Acceptance

1. `site/cli-help-blocks-scroll`: `task site-check` at 390 and 1024 px finds every help block on
   `/docs/reference/cli` with as many rendered lines as source lines, scrolling sideways.

## Scope

Inferred: the generated `website/docs/reference/cli.md` and its code-block style. Owner of the
change: the doc-writer.
