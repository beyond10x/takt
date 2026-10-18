---
format: aep.planning-md/3
id: story:merge-under-authority
kind: story
status: draft
title: Merge only under a recorded grant of repository.merge
summary: authority.yaml read into every evaluation; granted merges, absent stops ApprovalRequired
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:forge-pull-request-evidence
- depends_on: story:story-delivery-protocol
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/merge_under_authority.rs
revision: 10
---
# Story: merge-under-authority

## Outcome

The run merges a pull request exactly when the protocol admits `repository.merge` under a recorded authority decision, and otherwise stops at `ApprovalRequired (repository.merge)` with the pull request open and nothing merged; the operator grants with `takt approve`, a later run resumes the case and merges, and a connection to the forge is never authority.

## Context

`docs/design/takt.md` § 3: the loop runs "until an outcome is legitimate | an approval is needed | ..."; a deterministic step is "a merge on approval"; the `approval` block names a capability and an approver, `repository.merge` by the operator. In Canon an action that `requires` a capability is approval-required until a `canon-authority/1` decision grants it, and those decisions are supplied per evaluation (b10x-canon 0.6.0, `crates/canon/src/eval/mod.rs:319-326`; the document shape in `crates/canon/src/eval/authority.rs:7-21`: a list of `capability` and `decision: granted|denied`, one per capability). The kernel supplies none (`crates/takt/src/case.rs:185-189`), stops when only approval-required actions remain (`crates/takt/src/run.rs:194-201`), refuses to open a case id that exists (`crates/takt/src/case.rs:44-47`) and always opens a case (`crates/takt/src/run.rs:97-112`).

This story, following the cleared decision-blocker:approval-grant-and-case-resume: the run stops at the gate. A run whose case holds no grant stops at `ApprovalRequired (repository.merge)` with the pull request open and nothing merged, so an unattended run never merges without a grant. `takt approve <case> repository.merge` writes the case's `authority.yaml`, a `canon-authority/1` document granting `repository.merge`, and records an `approval` evidence on the story in the planning store (`aep plan artifact evidence <story> --kind approval --source takt --ref <case>/authority.yaml`); approvals may be given to several cases in a batch. The next `takt run` on the repository, or `takt run --case <id>`, resumes the case instead of opening one: the frontier finds the story `active` with an open case directory and offers that case, and a story has at most one open case, so it is never given a second. On resume `authority.yaml` is read on every evaluation and passed as `Supplied.authority`; with `repository.merge` granted the action is admissible, the merge is a deterministic step performed through the `Forge` (story:forge-pull-request-evidence) and so through the repository's bot route, and a `pr.merged` validator reads the pull request back and writes a `pull_request` record with result `merged`, after which `accepted` is legitimate and the run stops Completed. The decision document records the grant the way Canon explains it.

The `approval` block of `workflow/1` (`docs/design/takt.md` § 3: capability, approver) names the capability and the approver; the loader (story:workflow-schema-and-loader) hands it to this story's gate: the named capability becomes the run's gate, what `--gate` sets today (`crates/takt-cli/src/main.rs:66-68`, `crates/takt/src/run.rs:33-36`), and the approver is recorded with the grant, `takt approve <case> <capability> --by <approver>`, in `authority.yaml` and in the story's `approval` evidence. Whether the approver may approve is Mandate's and out of scope here.

## Domain relations

- AuthorityDecision → Case: one list per evaluation, at most one decision per capability. Inferred from b10x-canon 0.6.0 `crates/canon/src/eval/mod.rs:319-326` and `crates/canon/src/eval/authority.rs:7-21`; takt supplies none today, `crates/takt/src/case.rs:185-189`.
- Case → Protocol: exactly one. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:644-651`, entity `canon.protocol.Case`, relation `protocol`.
- Story → Case: at most one open case per story; a story `active` with an open case is resumed, never taken a second time. Decided in decision-blocker:approval-grant-and-case-resume.
- Story → evidence record (planning store): one `approval` record per grant, appended and never edited. Inferred from aep 0.74.0 `aep plan artifact evidence --help`.

## Scope

- `crates/takt/src/case.rs` (`authority.yaml` read into `Supplied.authority`; opening an existing case directory for resume)
- `crates/takt/src/run.rs` (a run resumed on an open case; a write-effect deterministic step; `Stop::Completed` after `pr.merged`; the gate taken from a workflow's `approval` block when one is loaded)
- `crates/takt/src/forge.rs` (`merge(number)`)
- `crates/takt/src/validator.rs` (`PrMerged`)
- `crates/takt/src/frontier.rs` (the `active` story with an open case offered for resume; the `approval` evidence written through the store adapter)
- `crates/takt-cli/src/main.rs` (`takt approve <case> <capability> --by <approver>`; `takt run --case <id>`)
- new: `crates/takt/tests/merge_under_authority.rs`

## Ordering

- depends_on story:forge-pull-request-evidence: both edit `crates/takt/src/case.rs`, `crates/takt/src/forge.rs`, `crates/takt/src/validator.rs` and `crates/takt-cli/src/main.rs`; the merge goes through the `Forge` that story adds.
- depends_on story:story-delivery-protocol: this story's runs read `protocols/story-delivery/1.yaml`; no shared file.
- story:frontier-next-ready-story and story:gate-validator come before this story through story:forge-pull-request-evidence; `crates/takt/src/frontier.rs`, `crates/takt/src/run.rs` and `crates/takt-cli/src/main.rs` are shared with them along that chain.

## Acceptance

Two runs on `protocols/story-delivery/1.yaml` with a forge double whose checks pass: the first, whose case holds no grant, stops at `ApprovalRequired (repository.merge)` with the pull request open and the double recording no merge; after `takt approve <case> repository.merge`, `takt run --case <id>` resumes that case, merges through the double, writes a `pull_request` record with result `merged`, and stops Completed with `accepted`.

## Checks

- `takt approve <case> repository.merge` writes `<case>/authority.yaml` as a `canon-authority/1` document with `repository.merge` granted.
- After `takt approve` on a case opened for a story, `aep plan artifact history <story>` shows one `approval` evidence record whose source names takt and whose reference is the case's `authority.yaml`.
- `takt run` on the repository with no `--case` and no `--intent`, while the story is `active` with an open case, resumes that case and opens no second one.
- A run with no grant, left alone under `--hours`, ends with nothing merged.
- The decision document of the resumed case shows the grant as what made `repository.merge` admissible.
- `takt approve <case> <capability> --by <approver>` records the approver beside the grant in `<case>/authority.yaml` and in the story's `approval` evidence record, and a run from a workflow whose `approval` block names a capability has that capability as its gate.

## Out of Scope

Whether the approver named in an `approval` block may approve (Mandate's); denying a capability other than by its absence; recording the approval anywhere but `authority.yaml` and the story's evidence; the end-to-end proof on a real repository (story:story-done-end-to-end).

## Ambiguities

- A `denied` decision: `inferable`, Canon treats it as not granted (`crates/canon/src/eval/authority.rs:7-12`); the run stops ApprovalRequired the same way.
- Which story an `approval` record lands on: `inferable`, the story the case was opened for (story:frontier-next-ready-story derives the case from the story); a case opened on an `--intent` has no story, and `takt approve` writes `authority.yaml` only.
- Resuming a case whose implementation revision moved on: `inferable`, the grant is read into the evaluation of the current revision, and Canon's claims about `implementation` are revision-bound, so a new commit returns `pr.opened` and `ci.green` to UNKNOWN and the merge is not admissible until they are re-established.
