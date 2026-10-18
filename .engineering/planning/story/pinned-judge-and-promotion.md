---
format: aep.planning-md/3
id: story:pinned-judge-and-promotion
kind: story
status: archived
title: A promoted takt judges takt's cases; judge surfaces need the owner's grant
summary: judge bundle from base, allow-list guard at merge, typed grants, revision in --version, promotion record
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scorecard-reader
revision: 2
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T10:03:42Z", actor: "human:timo", revision: 2}
---
# Story: pinned-judge-and-promotion

## Outcome

A case on takt itself is judged by a promoted takt, never by the candidate: the binary, the
protocol, the gates inputs and the planning store are read from the promoted base revision outside
the case's worktree; a merge whose `base..head` touches a path outside the allow-list needs a
`human:` grant; and a new binary is promoted only when the scorecard shows no regression, with a
promotion record naming the revision the binary was built from.

## Context

design:recursive-self-improvement § The judge, § Scorecard, § Roles. Today the protocol is loaded
from a path (`crates/takt/src/run.rs:22-23`), the gates inputs from the edited tree
(`crates/takt/src/validator.rs:193-216`), `takt approve --by` takes any string
(`crates/takt-cli/src/main.rs:49`), and `takt --version` prints a bare version with no revision.

## Domain relations

- Promotion → Scorecard: a promotion reads one scorecard. UNMAPPED: cardinality on the scorecard side.
- Promotion → Revision: a promotion names exactly one revision, the one the binary was built from.
- Grant → Approver: typed `human:<id>` or `agent:<role>`. Inferred from the design.

## Scope

- `crates/takt/src/case.rs` (typed grants) (inferred)
- `crates/takt/src/run.rs` (the merge-time guard over `base..head`) (inferred)
- `crates/takt-cli/src/main.rs` (`--by` parsing, `promote`, revision in `--version`) (inferred)
- new: `crates/takt-cli/build.rs` (the revision embedded at build) (inferred)
- new: `.engineering/judge-allow.yaml` (the allow-list: `docs/**`, `README.md`, `CHANGELOG.md`) (inferred)

## Ordering

- depends_on story:scorecard-reader: promotion reads the scorecard.
- depends_on story:validator-confinement, story:tree-hygiene and story:authority-hardening (epic:composable-agentic-work, on a branch not yet on main on 2026-10-18; the edges are added once it lands): the gates inputs from base and the grant bound to a revision are what this story extends.

## Acceptance

A case whose diff touches `crates/takt/src/validator.rs` refuses a merge granted `--by agent:operator`
and merges under `--by human:<id>`; a case whose diff touches only `docs/` merges under either; and
`takt promote` installs a binary only when the scorecard has no worse row, writing a record whose
revision equals the one `takt --version` prints.

## Checks

- A candidate that edits `.engineering/judge-allow.yaml` to admit its own paths is treated as a
  judge-surface change.
- A candidate branch's own `protocols/story-delivery/1.yaml` is not the protocol its case is judged by.

## Out of Scope

The self-improvement protocol (story:self-improvement-protocol); Substrate confinement
(story:confinement-seam).
