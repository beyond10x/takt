---
format: aep.planning-md/3
id: story:gitlab-forge
kind: story
status: active
title: takt delivers a story on a GitLab repository through glab
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/takt-cli/tests/gitlab_forge_cli.rs
- confidence: inferred
  path: crates/takt/src/forge/gitlab.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/forge.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T21:51:35Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T21:51:35Z", actor: "human:timo", revision: 4}
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
