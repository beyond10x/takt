---
format: aep.planning-md/3
id: story:judge-guard
kind: story
status: draft
title: The promoted takt judges takt's cases; a judge-surface merge needs the owner's grant or a delegation on record
summary: protocol and store from base, typed grants, allow-list and delegation record read at base, guard over base..head at merge
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
- supersedes: story:pinned-judge-and-promotion
- depends_on: story:validator-confinement
- depends_on: story:tree-hygiene
- depends_on: story:authority-hardening
scope:
- confidence: cited
  path: .engineering/delegations.yaml
- confidence: cited
  path: .engineering/judge-allow.yaml
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: inferred
  path: crates/takt/src/protocol.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/authority.rs
- confidence: inferred
  path: crates/takt/tests/authority_adv1.rs
- confidence: inferred
  path: crates/takt/tests/merge_under_authority.rs
revision: 11
---
# Story: judge-guard

## Outcome

A case on takt itself is judged by the promoted takt, never by the candidate: the promoted binary
reads the protocol and the planning store from the promoted base revision outside the case's
worktree; `takt approve --by` takes only `human:<id>` or `agent:<role>`; and a merge whose
`base..head` touches any path outside the allow-list `.engineering/judge-allow.yaml`, read at the
base revision, is refused unless its grant is the owner's (`human:`) or comes from a role that an
unrevoked delegation on record names.

The delegation record is `.engineering/delegations.yaml`, read at the base revision like the
allow-list: each entry names the delegating owner (`human:<id>`), the role (`agent:<role>`), the
capability (`repository.merge.judge_surface`), the instant it was given and, once revoked, the
instant of its revocation. The file is itself a judge surface, so adding, widening or revoking a
delegation is a change only the owner's grant merges. Until this story lands, a delegation is held
as the owner's decision in the planning store (product-requirements:takt § 2, recorded at
3ad0bd9).

## Context

design:recursive-self-improvement § The judge, § Roles. Today the protocol is loaded from a path
(`crates/takt/src/run.rs:22-23`), the gates inputs from the edited tree
(`crates/takt/src/validator.rs:193-216`; story:validator-confinement moves those to the base), and
`takt approve --by` takes any string (`crates/takt-cli/src/main.rs:49`). The judge is the binary
story:promotion installed, so this story follows it.

## Domain relations

- Grant → Approver: typed `human:<id>` or `agent:<role>`. Inferred from the design.
- Grant → Approver: takt.work.Approver; delegations takt.work.DelegationRecord; allow-list takt.work.JudgeAllowList (ess/domains/takt.yaml).

## Scope

Derived 2026-10-18 by `story-scoper`. Line numbers from `fix/2026-10-18-review` at 768db1c unless marked VC (`impl/validator-confinement` at c0e8f4e). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt` merge validator and grant path, plus the `approve` subcommand of `crates/takt-cli` — cited, the story names `validator.rs`, `run.rs:22-23` and `main.rs:49`
- **Files:** `crates/takt/src/validator.rs:676-791` (`PrMerged::observe`: the grant check at 764-784 and the forge merge at 786-787 are the only merge effect; the guard over `base..head` belongs beside them) — cited, read from the tree; the story placed the guard in `run.rs`
- **Files:** `crates/takt/src/validator.rs:193-216` (`GateRun::observe` reads gates inputs from the tree; VC moves them to `base_revision` at VC 194-214 and `file_at` at VC 274-285, which the allow-list read would reuse) — cited
- **Files:** `crates/takt-cli/src/main.rs:49-50` (`by: String`), `:197-208` (`approve` passes `by` straight to `frontier::approve`) — cited
- **Files:** `crates/takt/src/run.rs:23-24` (`RunOptions.protocol`), `:249-252` (protocol loaded from a file path when one exists) — cited
- **Files:** `.engineering/judge-allow.yaml`, new, holding `docs/**`, `README.md`, `CHANGELOG.md`; not present in `.engineering/` today — cited
- **Symbols:** `PrMerged::observe`, `Authority::grants_at` (`case.rs:338`), `ApproveArgs.by`, `RunOptions.protocol`, VC `Branch.base_commit` (VC `case.rs:88`) — cited
- **Also likely:** `crates/takt/src/case.rs:266-280,520-522,598-611` (`AuthorityDecision.approver: String`, the only approver check is non-empty, `prepare_grant` writes it) — inferred, typed grants checked in the library because tests call `grant()` directly
- **Also likely:** `crates/takt/src/protocol.rs:62-75` (`resolve` and `from_file` read `--protocol` from a path) and `crates/takt-cli/src/main.rs:262-265,521-528` — inferred, where "protocol from base" would change; the embedded built-in (`protocol.rs:33-37`) already comes from the binary's revision
- **Also likely:** `crates/takt/src/frontier.rs:231-240` (`Aep::plan` runs `aep` with `current_dir(repo)`, the case's tree) — inferred, where "store from base, outside the worktree" lands; tree-hygiene's `to_base` instead checks the base out in the same tree
- **Also likely:** `crates/takt/tests/merge_under_authority.rs`, `authority.rs`, `authority_adv1.rs` (about 30 calls grant as the bare `"operator"`) — inferred, they break once grants are typed
- **Documents:** `.engineering/judge-allow.yaml` only — cited
- **Confidence:** medium — the story cites three code sites and the tree confirms them, but the guard site differs from the story's guess and the store-from-base mechanism is not named
- **Would collide with:** any unit touching `PrMerged` or `GateRun` in `crates/takt/src/validator.rs`, `prepare_grant`/`AuthorityDecision`/`Branch` in `crates/takt/src/case.rs`, the case-open and protocol-load path in `crates/takt/src/run.rs`, the `approve` subcommand or argument struct in `crates/takt-cli/src/main.rs`, or the `Aep` adapter in `crates/takt/src/frontier.rs`
- **Safety fact:** the allow-list and the base of `base..head` are read at the case's `base_commit`, pinned when the case opened (VC `case.rs:84-88`, resolved by VC `validator.rs:194-214`, read by `file_at` at VC `validator.rs:274-285`), never from the tree or a branch name, so a candidate that edits `.engineering/judge-allow.yaml` cannot admit its own paths — level 2, unproven

Not established: the approver is free text in `authority.yaml` and the store evidence, a test asserts `--by operator` (`main.rs:1227`): whether typed grants still read old files; capability (`repository.merge.judge_surface` in `protocols/self-improvement/1.yaml:15-22,151-157`) or grant-type check; no mechanism for "store outside the case's worktree"; where `base..head` paths are computed; this story depends on validator-confinement and tree-hygiene being merged (line numbers move).

Typed entries: `crates/takt/src/validator.rs`, `crates/takt-cli/src/main.rs`, `crates/takt/src/run.rs`, `.engineering/judge-allow.yaml` (cited); `crates/takt/src/case.rs`, `crates/takt/src/protocol.rs`, `crates/takt/src/frontier.rs`, `crates/takt/tests/merge_under_authority.rs`, `crates/takt/tests/authority.rs`, `crates/takt/tests/authority_adv1.rs` (inferred).

## Ordering

- depends_on story:promotion: the judge is the promoted binary; both edit `crates/takt-cli/src/main.rs`, so this story follows it on that file.
- depends_on story:validator-confinement, story:tree-hygiene and story:authority-hardening (epic:composable-agentic-work, on a branch not yet on main on 2026-10-18; the edges are added once it lands): base-revision gates inputs and the grant bound to a revision are what this story extends.

## Acceptance

A case whose `base..head` touches `crates/takt/src/validator.rs` is refused at the merge when its
grant is `agent:planner` and `.engineering/delegations.yaml` at the base names no unrevoked
delegation of `repository.merge.judge_surface` to `agent:planner`; the same case merges when such a
delegation is on record, and is refused again once that delegation's revocation is on record.

## Checks

- The same case merges under `human:<id>`, with or without a delegation on record.
- A case whose diff touches only `docs/` merges under `agent:planner` with no delegation.
- A candidate that edits `.engineering/judge-allow.yaml` or `.engineering/delegations.yaml` to admit its own paths or its own role is treated as touching a judge surface, and its own edit of the delegation file does not count: the file is read at the base.
- A candidate branch's own `protocols/story-delivery/1.yaml` is not the protocol its case is judged by.
- A delegation naming another role (`agent:developer`) does not admit an `agent:planner` grant.

## Out of Scope

Promotion (story:promotion); Substrate confinement (story:confinement-seam).

## From the grant-subject review

From the reviewer verdict on story:grant-binds-to-action-subject at 938cc5a (approve, granted by the owner):

- `crates/takt/src/protocol.rs:195-225` finds a grant's subject by walking the raw protocol YAML for the evidence-reading forms Canon 0.6.0 has (`evidence`, `first`, `before`); a form a later Canon adds would be missed with no error. The judge should read the subject from Canon's typed predicates, or refuse a protocol whose forms it does not know.
- `crates/takt/src/case.rs:1517`: an edit to a case's `protocol.yaml` made outside a run is not detected; no digest pins it and the IR is recompiled from it (`protocol.rs:72`). The judge pins the protocol by digest at open and refuses a case whose copy differs.
