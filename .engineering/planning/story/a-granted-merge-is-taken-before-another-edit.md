---
format: aep.planning-md/3
id: story:a-granted-merge-is-taken-before-another-edit
kind: story
status: draft
title: A case resumed after a merge grant takes the merge before another edit
summary: the grant is used, not voided by an edit that moves the revision
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-granted-merge-is-taken-before-another-edit

## Outcome

A case resumed after `takt approve` granted `repository.merge` at its revision takes the merge
before any step that would move the revision, so the grant is used, not voided by another edit.

## Context

- Observed 2026-10-26 on takt 0.14.0 with `--harness codex --forge none`, written into the site's
  guide `website/docs/guides/approve-a-capability.md:70-84`: the resumed run's first `frontier:`
  line has `repository.merge (admissible)`, the model takes `implementation.edit` first, the
  revision moves, and step 3 stops `ApprovalRequired (repository.merge)` again; 45,753 tokens.
- A grant holds only at the case revision it names (`AGENTS.md` § Rules, `CaseAuthority` in
  `crates/takt/src/case.rs`), so an edit after it always asks for a new one: a loop of
  grant, edit, grant that a person cannot end except by not resuming.
- Where the preference lives is open: protocols are data (`AGENTS.md` § Rules), so offering only
  the granted action may belong in `story.delivery/1` rather than in takt's frontier.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check` with a
harness double that would propose `implementation.edit`.

1. `approval/resume-takes-the-granted-merge`: a case stopped `ApprovalRequired (repository.merge)`
   and granted at its revision is resumed; its first step is `repository.merge`, no harness call
   is made before it, and the merge is not refused for a missing grant.
2. `approval/edit-after-grant-is-not-offered`: on that resume the frontier offers no action that
   commits before the granted one; a case with no grant is offered what it is offered today.

## Scope

Inferred, not confirmed: `crates/takt/src/frontier.rs`, `crates/takt/src/run.rs`, the protocol
documents under `protocols/`, `crates/takt/tests/`.
