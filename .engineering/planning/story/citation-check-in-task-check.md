---
format: aep.planning-md/3
id: story:citation-check-in-task-check
kind: story
status: implemented
title: task check fails on a stale read-from citation in ess/
summary: the citation checker is committed and runs in task check
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: scripts/check-citations
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T14:01:46Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T14:01:47Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-18T15:23:35Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":3}}}
---
# Story: citation-check-in-task-check

## Outcome

`task check` fails when a `read from:` citation in `ess/` names a line that no longer says what it is cited for, so a stale citation is caught in the wave that makes it stale.

## Context

Decided 2026-10-18 on the developer role's report: 68 of 300 `read from:` citations in the ESS domain went stale during wave 07 unnoticed, because `task check` has no citation step and the checker the developer role used lives in a scratch directory outside the repository. Every wave edits cited code, and each wave close spends calls re-pointing citations by hand.

## Expected before

On takt at 7b5aca3, holds; after the fix, fails:

- Change a cited line in `crates/takt/src/run.rs` so the citation no longer matches, then run `task check`: it exits 0.

## Scope

- `Taskfile.yml`: a citation step in `check` (inferred)
- the checker: a script under `scripts/` or a test under `crates/takt-cli/tests/` (inferred; the developer role scopes it)
- `AGENTS.md`: the command under Commands (inferred)

## Acceptance

`task check` runs the citation checker; with one cited line changed it exits non-zero and names the citation and the line; on main it exits 0 with every citation resolving.

## Landed (wave 09)

Read from `git diff --stat 8ce2019 25d2978` at the close of wave 09 (wave branch head 1425689). Each line citation carries a digest of its cited text; `fix` only moves identical text; `accept` records a digest after a person read it. Later commits on its branch are wave merges and citation re-points.

- `.agents/developer-agent.md`
- `.agents/operator-agent.md`
- `AGENTS.md`
- `README.md`
- `Taskfile.yml`
- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/citation_check.rs`
- `crates/takt-cli/tests/citation_check_adv1.rs`
- `crates/takt-cli/tests/citation_check_adv2.rs`
- `crates/takt-cli/tests/scenario_help.rs`
- `crates/takt/tests/improvement_protocols.rs`
- `docs/design/takt.md`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
- `protocols/loop-improvement/1.yaml`
- `protocols/self-improvement/1.yaml`
- `scripts/check-citations`
