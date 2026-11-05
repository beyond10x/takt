---
format: aep.planning-md/3
id: approval-record:wave-54-adversary-correction
kind: approval-record
status: approved
title: 'Wave 54: the adversary''s findings'
summary: B2, C1, C2, B1 and B4 in one correction; D1 a draft
relations:
- decides: story:test-cmd-checked-before-open
- decides: story:resume-names-a-missing-branch
- decides: story:a-case-id-on-a-story-resume-is-refused
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-05T19:35:26Z", actor: "human:timo", revision: 2}
---
# Approval: wave 54's adversary findings

## Decision

The owner's standing delegation through the orchestrator, 2026-11-05, on the six red cases of wave 54's adversary
pass (`crates/takt/tests/w54_adversary.rs`, `crates/takt-cli/tests/w54_adversary_cli.rs`):

- One correction, committed with the adversary's two files, which must go green:
  - B2: on a resume, the test program is checked in the tree of `takt/<id>`, the tree the step runs in, not the tree
    as it is before the checkout (story:test-cmd-checked-before-open).
  - C1: a case branch kept only as `origin/takt/<id>` resumes, as the checkout before wave 54 did
    (story:resume-names-a-missing-branch).
  - C2: the printed `git branch` line and the case directory are shell-quoted (story:resume-names-a-missing-branch).
  - B1 (empty, `.` and relative `PATH` entries) and B4 (an execute bit for others only) join the correction because
    they mirror what the step's `execvp` and `access(X_OK)` do, without a new surface.
- D1, `--case-id` dropped when a `--story` or argument-less run resumes an open case, likely older than wave 54,
  becomes story:a-case-id-on-a-story-resume-is-refused, a draft.
- Clippy covers the adversary's files in the gate's `task check-static`.
