---
format: aep.planning-md/3
id: story:gitlab-forge
kind: story
status: draft
title: takt delivers a story on a GitLab repository through glab
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
## Outcome

A repository whose origin is a GitLab host runs `story.delivery` through `glab`: push through the bot route, merge request open, merge under a grant. `takt run` refuses before the first step, naming `glab`, unless `glab` is on PATH and `glab auth status` exits 0.

## Context

Owner decision of 2026-10-19: "later same thing with gitlab". takt has no GitLab forge today: `crates/takt/src/forge.rs` names GitLab only for credential scrubbing (`:383`). The trial project is the owner's choice (decision-blocker:gitlab-trial-project).

## Scope

- `crates/takt/src/forge.rs`: a second forge adapter (inferred)
- `crates/takt-cli/src/main.rs`: forge selection from the origin (inferred)

## Acceptance

Against the GitLab trial project, scenario 1 reaches the merge approval with one merge request; a run with `glab` absent or unauthenticated exits before the first step naming it.
