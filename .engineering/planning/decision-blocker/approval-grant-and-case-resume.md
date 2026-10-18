---
format: aep.planning-md/3
id: decision-blocker:approval-grant-and-case-resume
kind: decision-blocker
status: cleared
title: Nobody has decided how the operator grants the merge approval, nor whether a run waits for it or a later run resumes the case
relations:
- blocks: epic:composable-agentic-work
- blocks: story:week-of-daily-runs
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-18T02:06:36Z", actor: "human:timo", revision: 3}
---
# Decision blocker: approval-grant-and-case-resume

## Question

How does the operator grant `repository.merge` for a case, and does the run that needs it wait for the grant, or stop so that a later run resumes the case? The two halves are one question: the answer to the first fixes the second.

## Why nothing settles it

- `docs/design/takt.md` § 3 says the loop runs until "an approval is needed", that "a merge on approval" is a deterministic step, and that the `approval` block names a capability and an approver (`repository.merge` by the operator). It names who approves, not how the grant reaches the case, nor whether the run that opened the case is the one that merges.
- Canon reads authority as a `canon-authority/1` list supplied per evaluation (b10x-canon 0.6.0, `crates/canon/src/eval/mod.rs:319-326`, `crates/canon/src/eval/authority.rs:7-21`); it grants nothing and resolves no identity. The kernel supplies none (`crates/takt/src/case.rs:185-189`), refuses to open a case id that exists (`crates/takt/src/case.rs:44-46`), and has no path that resumes a case (`crates/takt/src/run.rs:90-116` always opens one).
- The planning store offers two more homes for a grant: `aep plan artifact evidence --kind approval` and the `approval-record` kind. The design says "AEP as the record" without saying whether the approval is one of those records.

## Options seen, not a recommendation

1. The run waits: it prints the pending approval and takes the grant from the terminal or from a file it watches, and the same process merges. One run per unit; nothing to resume; an unattended `--hours 2` run blocks on every merge.
2. The run stops. `takt approve <case> repository.merge` writes `authority.yaml` into the case; `takt run --case <id>`, or the frontier finding the story `active` with an open case, resumes it. Needs a resume path and a rule for a story with an open case.
3. The grant is a planning-store record on the story (`evidence --kind approval` or an `approval-record`) that the frontier reads into the case authority when it resumes the story. The audit trail is in the store; the case and the store must agree on which revision was approved.

Each option gives a different command line, a different Story → Case shape (whether a story may have more than one open case, and whether `active` means "has an open case"), and a different audit trail. The `approver` field of the workflow `approval` block is enforced differently under each.

## What this stops

- The end-to-end story of the epic's acceptance, one ready story finished with the merge approval the only manual step: not drafted. story:merge-under-authority covers the merge given a grant at case open; the path from a stopped run to a merged pull request is this question.
- story:week-of-daily-runs: cannot run before the answer, and counts interventions in whatever form the answer defines.
- The `approver` field in story:declared-loops: parsed and recorded, not enforced.

## Who decides

The operator of this repository, who wrote the design.

## Decision

Option 2, with option 3's record (the operator, 2026-10-18): the run stops at the gate with
`ApprovalRequired`. `takt approve <case> repository.merge` writes the case's `authority.yaml`
(a `canon-authority/1` grant) and records an `approval` evidence on the story in the planning
store. The next `takt run` on the repository, or `takt run --case <id>`, resumes the case: the
frontier finds the story `active` with an open case, reads the grant into the evaluation, and the
merge happens as a deterministic step through the bot route. A story has at most one open case.
An unattended run never merges without a grant; approvals may be given in a batch.
