---
format: aep.planning-md/3
id: story:git-reads-no-user-excludes
kind: story
status: draft
title: takt's git calls read no user excludes file
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
revision: 1
---
## Outcome

Every git call takt makes runs with `core.excludesFile` set to an empty value (and `$XDG_CONFIG_HOME/git/ignore` not read), so a file the user's or a test command's global excludes would hide is seen as untracked by takt's status, its `uncommitted` precondition and its step guard.

## Context

From the adversarial review of main at ae680d5 on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), finding A5.3, partly confirmed (medium). `NO_CONFIGURED_CODE` (`crates/takt/src/git.rs:26-44@ae680d5`) does not override `core.excludesFile`, `HOME` is passed through, and `Outside` omits git config. Replay: a global ignore entry `pkg/mod/` hides a shadow package (`git status -uall` prints nothing, import gives the shadow). From a harness step the hidden path appears as an ignored addition and is refused; from a test command the excludes file and the shadow survive, and the next `tests.run` passes the `uncommitted` check (`run.rs:3463`) while importing the shadow (inferred end to end).

## Scope

- `crates/takt/src/git.rs`: `command`, `NO_CONFIGURED_CODE` (cited)

## Acceptance

With `$HOME/.config/git/ignore` listing `pkg/mod/` and an untracked `pkg/mod/__init__.py`, takt's `uncommitted` reports `pkg/mod/__init__.py`.
