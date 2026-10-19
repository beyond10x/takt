---
format: aep.planning-md/3
id: story:judge-guard
kind: story
status: active
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
  path: crates/takt/src/forge.rs
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
revision: 26
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T06:11:27Z", actor: "human:timo", revision: 24}
- {from: "proposed", to: "active", at: "2026-10-19T06:11:27Z", actor: "human:timo", revision: 25}
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

Derived 2026-10-19 by `story-scoper`, read-only at `wave/2026-10-19-w13` bbe5bdb: main e5f1ae1 plus E1 (check-finding), E2 (promote and scorecard), propfuzz, scan-before-push and `impl/loop-scorecard` f34c923, which bbe5bdb merges. `crates/takt/src/run.rs`, `protocol.rs` and `frontier.rs` are unchanged between e5f1ae1 and bbe5bdb. Line numbers are `path:line@rev`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** the `repository.merge` validator `PrMerged` in `crates/takt/src/validator.rs`, the grant path in `crates/takt/src/case.rs`, and the `approve` subcommand of `crates/takt-cli` — cited, read at bbe5bdb
- **Files:** `crates/takt/src/validator.rs:1473-1815@bbe5bdb` (`PrMerged`). In `observe_waiting` (1563-1814) the grant check `Authority::read` + `grants_at(self.action(), revision)` is at 1623-1645; wave 13's scan-before-push puts `scan_outgoing` and `push_revision` at 1646-1700, and the merge, the only merge effect, is `self.forge.merge(..)` at 1729-1732. The guard belongs between 1645 and 1646, before anything is pushed — cited
- **Files:** `crates/takt/src/validator.rs:101-192@bbe5bdb` (`Judged`, `judged`; the `diff-tree -r -z --no-renames --name-only <at> <revision>` call at 155-167), `base_revision` at 529-554, `file_at` at 611-624, `carries` at 892-909 — cited
- **Files:** `crates/takt-cli/src/main.rs:274-286@bbe5bdb` (`ApproveArgs`, `by: String` at 285) and 468-490 (the `Commands::Approve` arm passes `by` to `frontier::approve` at 479); `Forged::register` at 1015-1027 registers `PrMerged` only when a forge exists (1024); `--forge none` gives `forge: None` (1047-1053) — cited
- **Files:** `.engineering/judge-allow.yaml`, new, holding `docs/**`, `README.md`, `CHANGELOG.md`, and `.engineering/delegations.yaml` (new); `git ls-tree bbe5bdb .engineering/` lists only `cases`, `evidence`, `planning`, `project.yaml` — cited
- **Symbols:** `PrMerged::observe_waiting`; in `case.rs@bbe5bdb`: `Authority::read` (532-542), `Authority::grants_at` (562-566), `AuthorityDecision` (488-504, `approver: String` at 494), `Approval.approver` (612), `APPROVER_KINDS`/`is_approver` (667-681), `prepare_grant` (888-1022: typed approver for a story's case only at 916-922, any non-empty text on an intent case at 909-911), `Branch.base_commit` (418, pinned at open 1637-1640), `approvals_only` (1282-1382); plus `ApproveArgs.by`, `judged`, `base_revision`, `file_at` — cited
- **Also likely:** `crates/takt/src/forge.rs`: `PullRequestView` (258-266@bbe5bdb) holds state and head only, and `trait Forge` (293-332) has no method that reads the target's tip; wave 13 added `BotRoute::remote_base` (970-1005), a `fetch --dry-run` through the bot route (argv 950-968) that reads a remote branch's commit, private to `BotRoute`. D3's and D4's "target's tip as the forge reports it" needs a trait method; a default method avoids touching the 13 test `impl Forge` blocks in 10 files — inferred
- **Also likely:** `crates/takt/src/run.rs@bbe5bdb`: `JudgeSurface`, `JUDGE_LIST`, `parse_judge_list`, `judge_list_at` at 1883-1940 (the base-read glob list an allow-list parser sits beside or reuses), `JUDGE_SURFACE` at 927, the case-open protocol load at 3156-3160 — inferred
- **Also likely:** `crates/takt/src/protocol.rs@bbe5bdb` (`resolve`, `from_file` at 62-76; `subjects_of` at 192-229); `case.rs:1733@bbe5bdb` (`CaseDir::resume` recompiles the case's own `protocol.yaml`); `case.rs:1850` (open writes `protocol.yaml`, no digest); `main.rs:1509-1520@bbe5bdb` (`case_protocol`) — inferred, where "a candidate's own protocol is not its judge" and the digest pin land
- **Also likely:** `crates/takt/src/frontier.rs:480-490@bbe5bdb` (`Aep::plan_as`, `current_dir(repo)`) and `run.rs:3058-3088@bbe5bdb` (`to_base`) — inferred, where "store from base, outside the worktree" lands
- **Also likely:** `crates/takt/tests/merge_under_authority.rs`, `authority.rs`, `authority_adv1.rs`; 32 bare `grant(..., "operator"|"owner")` calls over 6 test files at bbe5bdb — inferred, they break only if typed approvers extend to intent cases
- **Also likely:** the two new files are parsed through `crate::loop_scorecard::read_yaml` (`loop_scorecard.rs:567@bbe5bdb`, which refuses anchors and aliases) — inferred, a call only
- **Documents:** `.engineering/judge-allow.yaml`, `.engineering/delegations.yaml`; `AGENTS.md:68-76@bbe5bdb` and `.agents/planner-agent.md:54-62@bbe5bdb` already state the rule — cited
- **Confidence:** medium — the merge site, the grant check and the approver typing are read in the tree; the target-tip read, the store-from-base mechanism and the protocol digest pin do not exist at bbe5bdb
- **Would collide with:** any unit touching `PrMerged`, `judged`, `base_revision` or `file_at` in `validator.rs`; `Authority`, `AuthorityDecision`, `is_approver` or `prepare_grant` in `case.rs`; `ApproveArgs`, the `Approve` arm or `Forged::register` in `main.rs`; `trait Forge`, `PullRequestView` or `BotRoute::remote_base` in `forge.rs`; `JudgeSurface`/`parse_judge_list` or the case-open protocol load in `run.rs`; the `Aep` adapter in `frontier.rs`. story:every-yaml-read-refuses-aliases changes `Authority::read`, `Branch::read`, `parse_judge_list` and the `protocol.rs` reads: it follows this story on those files
- **Safety fact:** under D3 the allow-list and the delegation record are read at the merge target's tip as the forge reports it, never from the tree; a step cannot write either file, `.engineering/**` except `cases/**` being on the step judge surface (`run.rs:927@bbe5bdb`); the remote read writes nothing (`fetch --dry-run`, `forge.rs:950-968@bbe5bdb`) — level 2, unproven

## Planner's decisions at scoping (2026-10-19)

- D7 (target tip): `trait Forge` gains a default method returning the merge target's tip as the forge reports it; the default refuses ("this forge cannot report the target's tip"), so a forge that has not implemented it never merges a judge-surface change; `BotRoute` implements it through `remote_base`.
- D8 (`--forge none`): the CLI registers no merge validator without a forge, so D3's `--forge none` clause has nothing to guard there; the guard is tested through the `Memory` double.
- D9 (documents): the unit changes `AGENTS.md:72-76` from "read at the base revision" to "read at the merge target's tip as the forge reports it", one sentence, so the rule and the code say the same.

## Ordering

- depends_on story:promotion: the judge is the promoted binary, and both edit `crates/takt-cli/src/main.rs` (promotion adds `Promote`, `PromoteArgs` and its arm at `main.rs:52-89,395-435@2d10b7f`, near `ApproveArgs` and the `approve` arm at 166-178 and 348-359@2d10b7f), so this story follows it on that file.
- depends_on story:validator-confinement, story:tree-hygiene and story:authority-hardening (epic:composable-agentic-work, all three on main): gates inputs read at the pinned base, the store read from the base and a grant bound to one revision are what this story extends. Wave 12 (`wave/2026-10-18-w12` at 76be9ed) also lands first and renumbers `validator.rs`, `case.rs`, `run.rs` and `main.rs`, so this story is cut from main after both.

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

The planner's decisions on the questions the 2026-10-19 scope left open, inside the design this story states (revised after the reviewer's verdict on them, 2026-10-19):

- D1, the guarded range: the guard diffs `base_commit..<granted revision>`, the range `judged` already computes (`validator.rs:140-152@1ded28f`). This is the design's `base..head` (`recursive-self-improvement.md:111`) split at the grant: `granted..head` may carry only approval records, which `carries`/`approvals_only` already enforce (`validator.rs:1374,1437-1442@1ded28f`, `case.rs:1067@1ded28f`), so the guard need not judge it. A path in `granted..head` that `approvals_only` does not admit stops the merge as today.
- D2, allow-list and deny-list: `.engineering/judge-allow.yaml` gates only the merge; the existing deny-list `.engineering/judge-surface.yaml` keeps gating what steps and validators may write. Neither replaces the other.
- D3, which revision the lists are read at: the allow-list and `.engineering/delegations.yaml` are read at the merge target's tip as the forge reports it when the merge is decided, never at `base_commit` and never from the tree. Under `--forge none` the target's tip is the local base ref as recorded by the step checkpoint (`Checkpoint::take_guarding`, `git.rs:1075@df7bddf`, over the refs `kept_branches` lists, `run.rs:2840-2847@df7bddf`), not as read at merge time, since a test command can move the local ref. "The promoted takt" is the binary named by story:promotion's latest record, which runs the case.
- D4, the base: the guard refuses the merge when `base_commit` is not an ancestor of the merge target's tip as the forge reports it. `branch.yaml` is the only copy of `base_commit` (`case.rs:1427@1ded28f`), so no comparison with "the value pinned at open" is possible; the target's tip is the independent fact.
- D5, untyped approvers: an `authority.yaml` approver that is neither `human:` nor `agent:` grants nothing at a judge-surface merge; on any other merge it reads as before.

Acceptance, added: a case whose `base_commit` is set, outside the loop, to a candidate commit that adds `crates/**` to `.engineering/judge-allow.yaml` is refused at the merge naming the base, under an `agent:planner` grant with no delegation; a case whose `granted..head` holds only approval records merges as before.
