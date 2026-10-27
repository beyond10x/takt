---
format: aep.planning-md/3
id: epic:downstream-story-delivery
kind: epic
status: draft
title: takt delivers a story on a repository that is not ours, the way that repository defines done
summary: the gaps a downstream trial found around takt's inner loop
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# Epic: downstream-story-delivery

## Outcome

`takt run` on a repository that is not ours delivers one story the way that repository defines done:
- it pays for no session on a harness binary that cannot pass the floor;
- it reads the repository's instructions and obligations from the base;
- it keeps its case records through worktree cleanup;
- it writes commits in the repository's style with the story reference;
- it prepares the repository with declared setup;
- it opens its change request through a forge the repository can configure: exact commands, the GitLab API, or
  optionally Connectors.

## Context

A downstream trial (2026-10-27) ran three trials of one small story on takt 0.14.0 with `--forge none` and Claude Code
2.1.309. All three stopped at exit 7 `Unperformable (pr.opened)`.
- The inner loop held: test first, red then green from validators, an honest stop.
- What failed was around it:
  - Handover without the repository's changelog, release-note and documentation changes.
  - A paid session on an unqualified binary (2.1.311, no H11 canary; 37,234 tokens).
  - A base under `takt/` refused as a case branch.
  - Ignored case records deleted by `worktree finish --discard-cache`.
  - Commit subjects `<action>: <model summary>` with no story reference.
  - A manual enter/leave lifecycle.
  - A gate command bound to one host's absolute paths.

## Stories

- `story:an-unqualified-harness-stops-before-the-first-session`
- `story:a-takt-prefixed-base-is-named-as-reserved`
- `story:a-cases-root-survives-worktree-cleanup`
- `story:every-harness-reads-the-repository-instructions-from-the-base`
- `story:step-commits-follow-a-configurable-style`
- `story:the-run-shows-each-repository-obligation`
- `story:harness-program-is-chosen-per-run`
- `story:declared-setup-runs-before-the-first-check`
- `story:a-command-forge-runs-the-configured-commands`
- `story:the-gitlab-forge-can-call-the-api-with-a-named-credential`
- `story:a-connectors-forge-adapter` (low priority)

Related outside this epic:
- `epic:protocol-settings-in-the-run-file` (red-first and gate deferral as settings).
- `story:the-workspace-seam-enters-and-leaves-managed-worktrees` under `epic:continuous-orchestration` (the outer
  lifecycle).
- The amended `story:story-delivery-tests-first`.

## Delivery rule

There is no intermediate release or pin hop between cross-repository stories. A chain (metaharness -> loom -> takt;
engineering-protocols -> takt; worktree -> takt) is delivered in one workspace:
- Each downstream `Cargo.toml` gets an uncommitted `[patch]` path override to the upstream working tree. For
  `worktree`, the locally built binary goes on `PATH`.
- The end-to-end check runs on that workspace.
- Then the releases go out in chain order, as pin-only hops.

## Acceptance

A `takt run` on a fixture repository whose base has an `AGENTS.md` commit rule, a gates document with a change-aware
release-note gate, a declared setup and a fake forge ends in an opened change request. Its commits carry
`Refs: story:<id>`, its prompt named the obligation, and its case directory survives `worktree finish --discard-cache
--archive`.

## Out of scope

- OS confinement (`story:harness-child-confined`).
- The live run against a downstream forge. It waits for the operator's identity choice.
