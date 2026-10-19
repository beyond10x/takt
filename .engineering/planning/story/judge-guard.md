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
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: inferred
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/authority.rs
- confidence: inferred
  path: crates/takt/tests/authority_adv1.rs
- confidence: inferred
  path: crates/takt/tests/merge_under_authority.rs
revision: 18
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

Derived 2026-10-19 by `story-scoper`, read-only at `main` 3e87bca (code unchanged at a388adf, which changes only the planning store), `headstart/promotion` df9681a and `wave/2026-10-18-w12` 3b0b748. Line numbers are `path:line@rev`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** the `repository.merge` validator in `crates/takt` (`PrMerged`), the grant path in `crates/takt/src/case.rs`, and the `approve` subcommand of `crates/takt-cli` — cited, read from the tree at all three revisions
- **Files:** `crates/takt/src/validator.rs:1349-1518@3e87bca` (`PrMerged::observe_waiting`: the grant check `grants_at(self.action(), revision)` at 1407-1431 and the forge merge at 1494-1497 are the only merge effect; at 3b0b748 the check is 1438 and the merge 1523-1526, and the wave adds `PrMerged::ACTION`/`KIND` at 1324-1330); the guard belongs beside the grant check — cited
- **Files:** `crates/takt/src/validator.rs:87-177@3e87bca` (`judged`: `git diff-tree -r -z --no-renames --name-only <base_commit> <revision>` at 140-152, already used by `tests.run` and `ci.read`; 122 and 154-166@3b0b748), `base_revision` at 437-458 and `file_at` at 517-528 (415 and 495@3b0b748), which the allow-list and delegation reads would reuse — cited
- **Files:** `crates/takt-cli/src/main.rs:120-131@3e87bca` (`ApproveArgs.by: String`) and `:301-312` (the `approve` arm passes `by` straight to `frontier::approve`); 122-133 and 312-323@3b0b748; 166-178 and 348-359@df9681a — cited
- **Files:** `.engineering/judge-allow.yaml`, new, holding `docs/**`, `README.md`, `CHANGELOG.md`, and `.engineering/delegations.yaml`, new; neither exists at any of the three revisions (`git ls-tree .engineering/` lists only `cases`, `evidence`, `planning`, `project.yaml`) — cited
- **Symbols:** `PrMerged::observe_waiting`, `Authority::grants_at` (`case.rs:347@3e87bca`, 432@3b0b748), `AuthorityDecision.approver` (279; 364@3b0b748), `is_approver`/`APPROVER_KINDS` (452-466; 545@3b0b748), `prepare_grant` (673-707; 758@3b0b748), `ApproveArgs.by`, `judged`, `base_revision`, `file_at`, `Branch.base_commit` (`case.rs:203`; 288@3b0b748) — cited
- **Also likely:** `crates/takt/src/run.rs:2581-2585@3e87bca` (case open loads `options.protocol` from a file when one exists; 3146-3149@3b0b748) and `JudgeSurface`/`JUDGE_LIST`/`judge_list_at` at 1313-1370 (1878-1930@3b0b748), the existing base-read glob list an allow-list matcher would sit beside — inferred, the story cites `run.rs` but no edit there is forced
- **Also likely:** `crates/takt/src/protocol.rs:62-76` (`resolve`, `from_file` read a path; the built-in `EMBEDDED` at 33-37 already comes from the binary's revision), `case.rs:1523@3e87bca` (resume recompiles the case's own `protocol.yaml` inside the tree; 1615@3b0b748) and `main.rs:1205-1211@3e87bca` (`case_protocol`) — inferred, where "a candidate's own protocol is not its judge" lands
- **Also likely:** `crates/takt/src/frontier.rs:480-490` (`Aep::plan_as` runs `aep` with `current_dir(repo)`, the case's tree; unchanged at all three revisions); `run.rs:2482-2500@3e87bca` `to_base` checks the base branch out in the same tree — inferred, where "store from base, outside the worktree" lands
- **Also likely:** `crates/takt/tests/merge_under_authority.rs` (14 bare grants; +213 lines in wave 12), `authority.rs` (8), `authority_adv1.rs` (1); 32 single-line `grant(..., "operator"|"owner")` calls over 6 test files at 3e87bca and 3b0b748 — inferred, they break only if typed approvers extend to intent cases
- **Documents:** `.engineering/judge-allow.yaml` and `.engineering/delegations.yaml`; `AGENTS.md:65-74@3e87bca` and `.agents/planner-agent.md:57-62@3e87bca` already state the rule from this story on and need no edit — cited
- **Confidence:** medium — the merge site, the pinned-base read and the existing approver check are read in the tree; the store-from-base mechanism, how takt's riders are excluded from `base..head`, and how the allow-list relates to the existing deny-list are not (the planner's decisions below settle the three)
- **Would collide with:** any unit touching `PrMerged`, `judged`, `base_revision` or `file_at` in `crates/takt/src/validator.rs`; `Authority`, `AuthorityDecision`, `is_approver` or `prepare_grant` in `crates/takt/src/case.rs`; `ApproveArgs`, the `Approve` arm, `Forged::register` (712-723@3e87bca) or the protocol default (374-386@3e87bca) in `crates/takt-cli/src/main.rs`; `JudgeSurface` or the case-open protocol load in `crates/takt/src/run.rs`; the `Aep` adapter in `crates/takt/src/frontier.rs`
- **Safety fact:** the allow-list and the delegation record are read with `file_at` at the case's `base_commit` (`validator.rs:517-528,437-458@3e87bca`), pinned at open (`case.rs:1424-1430@3e87bca`), never from the tree, so a candidate's own edit of either is just another `base..head` path outside the allow-list — level 2, unproven; `base_commit` itself lives in the case's tracked `branch.yaml` under `.engineering/cases/**`, which is off the step judge surface (`run.rs:357@3e87bca`), and `base_revision` only `rev-parse`s it with no ancestry check

Not established: the approver is free text in `authority.yaml` — partly answered: a story's case already takes only `human:`/`agent:` at grant time (`case.rs:700-707,460-466@3e87bca`) and the CLI test asserts `--by human:operator` (`main.rs:1877-1895@3e87bca`), but an intent case takes any non-empty text (694-696), `AuthorityDecision.approver` is still `String` (279), `Authority::read` (317-327) checks nothing and `PrMerged` checks only `grants_at`, so an old untyped file still reads and still merges. Capability or grant-type check — partly answered: self-improvement's `candidate.merge_judge_surface` requires `repository.merge.judge_surface` (`protocols/self-improvement/1.yaml:162-176@3e87bca`; loop-improvement 131), but the opener declares `surface` as a case input (16-22) instead of having it computed, and story-delivery's merge requires only `repository.merge` (`protocols/story-delivery/1.yaml:145-146@3e87bca`, 200-201@3b0b748). Store outside the case's worktree — not answered: no mechanism at any revision; promotion records the promoted revision (`promotion.rs:113-144,490-494@df9681a`) but only `takt promote` reads it (`main.rs:405-421@df9681a`). Where `base..head` paths are computed — partly answered: `judged` diffs `base_commit..revision` (`validator.rs:140-152@3e87bca`) and `PrMerged` computes none; the merged head carries takt's riders in `.engineering/planning/`/`.engineering/evidence/` (`frontier.rs:337@3e87bca`, `approvals_only` at `case.rs:1067`) and the case directory. Dependency on validator-confinement and tree-hygiene — answered: both are on main. New: main already has a deny-list judge surface (`JUDGE_SURFACE`, `.engineering/judge-surface.yaml`, `run.rs:357,1327@3e87bca`) for steps and validators.

Typed entries: `crates/takt/src/validator.rs`, `crates/takt-cli/src/main.rs`, `crates/takt/src/case.rs`, `.engineering/judge-allow.yaml`, `.engineering/delegations.yaml` (cited); `crates/takt/src/run.rs`, `crates/takt/src/protocol.rs`, `crates/takt/src/frontier.rs`, `crates/takt/tests/merge_under_authority.rs`, `crates/takt/tests/authority.rs`, `crates/takt/tests/authority_adv1.rs` (inferred).

## Ordering

- depends_on story:promotion: the judge is the promoted binary, and both edit `crates/takt-cli/src/main.rs` (promotion adds `Promote`, `PromoteArgs` and its arm at `main.rs:52-89,395-435@df9681a`, near `ApproveArgs` and the `approve` arm at 166-178 and 348-359@df9681a), so this story follows it on that file.
- depends_on story:validator-confinement, story:tree-hygiene and story:authority-hardening (epic:composable-agentic-work, all three on main): gates inputs read at the pinned base, the store read from the base and a grant bound to one revision are what this story extends. Wave 12 (`wave/2026-10-18-w12` at 3b0b748) also lands first and renumbers `validator.rs`, `case.rs`, `run.rs` and `main.rs`, so this story is cut from main after both.

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

## Planner decisions

The planner's decisions on the questions the 2026-10-19 scope left open, inside the design this story states:

- D1, the guarded range: the guard diffs `base_commit..<granted revision>`, the range `judged` already computes (`validator.rs:140-152@3e87bca`), not `base..head` of the merged branch. takt's own riders (`.engineering/planning/`, `.engineering/evidence/`, the case directory) are written after the grant and are not the candidate's change; a grant is bound to one revision (story:authority-hardening), so the range it judges ends there.
- D2, allow-list and deny-list: `.engineering/judge-allow.yaml` gates only the merge; the existing deny-list `.engineering/judge-surface.yaml` keeps gating what steps and validators may write. Neither replaces the other.
- D3, which base: the allow-list and `.engineering/delegations.yaml` are read at the case's pinned `base_commit`; "the promoted takt" means the binary named by story:promotion's latest record, which runs the case; the planning store is read at `base_commit` too.
- D4, a moved base: the guard refuses the merge when `base_commit` is not an ancestor of the granted revision, or differs from the value pinned at open. Whether a step can repoint `base_commit` today (`branch.yaml` under `.engineering/cases/**`, off the step judge surface, `run.rs:357@3e87bca`) is not verified; the acceptance below holds either way.
- D5, untyped approvers: an `authority.yaml` approver that is neither `human:` nor `agent:` grants nothing at a judge-surface merge; on any other merge it reads as before.

Acceptance, added: a case whose step rewrites `base_commit` in `branch.yaml` to the granted revision's parent is refused at the merge naming the base; a case whose only change outside the allow-list is takt's own riders merges under an `agent:planner` grant with no delegation.
