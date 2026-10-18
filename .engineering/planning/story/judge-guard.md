---
format: aep.planning-md/3
id: story:judge-guard
kind: story
status: draft
title: The promoted takt judges takt's cases; a judge-surface merge needs a human grant
summary: protocol and store from base, typed grants, allow-list guard over base..head at merge
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
- supersedes: story:pinned-judge-and-promotion
- depends_on: story:validator-confinement
- depends_on: story:tree-hygiene
- depends_on: story:authority-hardening
revision: 1
---
# Story: judge-guard

## Outcome

A case on takt itself is judged by the promoted takt, never by the candidate: the promoted binary
reads the protocol and the planning store from the promoted base revision outside the case's
worktree; `takt approve --by` takes only `human:<id>` or `agent:<role>`; and a merge whose
`base..head` touches any path outside the allow-list `.engineering/judge-allow.yaml`, read at the
base revision, is refused unless its grant is `human:`.

## Context

design:recursive-self-improvement § The judge, § Roles. Today the protocol is loaded from a path
(`crates/takt/src/run.rs:22-23`), the gates inputs from the edited tree
(`crates/takt/src/validator.rs:193-216`; story:validator-confinement moves those to the base), and
`takt approve --by` takes any string (`crates/takt-cli/src/main.rs:49`). The judge is the binary
story:promotion installed, so this story follows it.

## Domain relations

- Grant → Approver: typed `human:<id>` or `agent:<role>`. Inferred from the design.

## Scope

- `crates/takt/src/case.rs` (typed grants) (inferred)
- `crates/takt/src/run.rs` (the merge-time guard over `base..head`; protocol and store from base) (inferred)
- `crates/takt/src/validator.rs` (base-revision reads beside the gates inputs) (inferred)
- `crates/takt-cli/src/main.rs` (`--by` parsing) (inferred)
- new: `.engineering/judge-allow.yaml` (`docs/**`, `README.md`, `CHANGELOG.md`) (inferred)

## Ordering

- depends_on story:promotion: the judge is the promoted binary; both edit `crates/takt-cli/src/main.rs`, so this story follows it on that file.
- depends_on story:validator-confinement, story:tree-hygiene and story:authority-hardening (epic:composable-agentic-work, on a branch not yet on main on 2026-10-18; the edges are added once it lands): base-revision gates inputs and the grant bound to a revision are what this story extends.

## Acceptance

A case whose `base..head` touches `crates/takt/src/validator.rs` is refused at the merge when its
grant is `agent:operator`.

## Checks

- The same case merges under `human:<id>`.
- A case whose diff touches only `docs/` merges under `agent:operator`.
- A candidate that edits `.engineering/judge-allow.yaml` to admit its own paths is treated as touching a judge surface.
- A candidate branch's own `protocols/story-delivery/1.yaml` is not the protocol its case is judged by.

## Out of Scope

Promotion (story:promotion); Substrate confinement (story:confinement-seam).
