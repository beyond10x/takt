---
format: aep.planning-md/3
id: story:gitlab-forge
kind: story
status: implemented
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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T21:51:35Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T21:51:35Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T23:58:32Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":2,"review_outcome":1}}}
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

## Delivered

Wave 36 (docs/waves/wave-36.md): U1 at 2d0239f, the review's fixes at db65aad. `--forge gitlab`
(`crates/takt/src/forge/gitlab.rs`, `Glab`) pushes the case's branch over HTTPS with `glab auth
git-credential` as its one credential helper, opens the merge request, reads its state and head
pipeline and merges it under a grant through `glab api`; without `--forge`, an `origin` off
github.com gets `gitlab` when `glab auth status --hostname <host>` exits 0, else `none` with
glab's one line; `glab` absent or not logged in is refused before the case opens, naming it. The
acceptance's real run: one case on the private trial project (kept outside the repository), on
`story.delivery/1` with `--harness claude --forge gitlab`, reached step 8 `pr.open`, which pushed
the case branch through glab and opened one merge request, and stopped `ApprovalRequired
(repository.merge)`, exit 4, at step 52; the merge request was closed unmerged and its branches
deleted. The merge request's files keep the bot route's `pull-request.*` names; a scenario run
refuses `gitlab` (review finding F6). Open edges: story:gitlab-forge-hardening.
