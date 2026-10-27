---
format: aep.planning-md/3
id: approval-record:trial-review-d89-wave
kind: approval-record
status: accepted
title: 'Records D89: the trial-review stories are implemented in parallel units'
tags:
- wave-approval
relations:
- decides: story:a-cases-root-survives-worktree-cleanup
- decides: story:a-command-forge-runs-the-configured-commands
- decides: story:a-connectors-forge-adapter
- decides: story:a-takt-prefixed-base-is-named-as-reserved
- decides: story:an-unqualified-harness-stops-before-the-first-session
- decides: story:declared-setup-runs-before-the-first-check
- decides: story:every-harness-reads-the-repository-instructions-from-the-base
- decides: story:harness-program-is-chosen-per-run
- decides: story:step-commits-follow-a-configurable-style
- decides: story:takt-lists-a-protocols-settings
- decides: story:takt-ships-default-protocol-settings
- decides: story:the-gitlab-forge-can-call-the-api-with-a-named-credential
- decides: story:the-run-file-sets-protocol-settings
- decides: story:the-run-shows-each-repository-obligation
- decides: story:the-workspace-seam-enters-and-leaves-managed-worktrees
- decides: story:story-delivery-tests-first
revision: 2
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T12:00:38Z", actor: "human:timo", revision: 2}
---
Decider: orchestrator, relayed to this coordinator.

Records decision D89, 2026-10-27, as the orchestrator sent it:

"D89: option A in parallel, nothing paused: one opus coordinator for the 15 trial-review stories (takt +
engineering-protocols + worktree + metaharness), fans out sonnet implementors, up to 4 at once, units by file overlap,
targeted tests (D85), merge at green into integration/takt / plan branches, reviews after; at most 2 takt trees
building at once, small repos free."

The owner's words it acts on, 2026-10-27: "So why can't you do this in parallel? Speed up your work, your subagents.
Be faster. It's unacceptable to wait so long for these small changes."

What it means for the store:

- The fifteen trial-review stories of epic:downstream-story-delivery, epic:protocol-settings-in-the-run-file and
  epic:continuous-orchestration move to active, with story:story-delivery-tests-first (already active).
- Units are grouped by file overlap. `crates/takt/src/run.rs` and `crates/takt/src/git.rs` are hot: one unit editing
  each at a time; units on other files run in parallel, at most two takt trees building at once.
- A story whose dependency-blocker names another repository is built against that repository's working tree through
  an uncommitted `[patch]` path override, and merges here once the upstream release it needs is pinned.
- Each unit runs its story's test files, the changed modules' unit tests, fmt, clippy on the changed crates and the
  cheap checks, commits at green and merges into integration/takt. Adversary passes run after the merge and land
  their fixes on top. No pull request to main, tag or release of takt.
- The forge adapters are built and tested on fakes only; the Connectors forge comes last.
