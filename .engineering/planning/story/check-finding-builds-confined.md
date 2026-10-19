---
format: aep.planning-md/3
id: story:check-finding-builds-confined
kind: story
status: implemented
title: check-finding builds the revision it checks without the operator's home or environment
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
scope:
- confidence: inferred
  path: crates/takt-cli/tests/finding_scripts.rs
- confidence: cited
  path: scripts/check-finding
- confidence: inferred
  path: scripts/file-finding
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T01:21:29Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T01:21:29Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-19T06:14:32Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`scripts/check-finding`'s `cargo build` of the revision under check runs with a scratch HOME, an environment allow-list and no write access outside its own build directory, as the clone and the predicate already do; a build of a candidate cannot read the operator's credentials or overwrite another revision's cached build.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding S9.2, confirmed: `scripts/check-finding:233-234@9cc158b` runs `cargo build` with the caller's whole environment and real HOME, so a candidate's `build.rs`, proc macros and `.cargo/config.toml` run unconfined; only the clone (`:265-269`) and the predicate (`:287`) get `env -i` and a fresh HOME. Inferred, not run: a candidate `build.rs` could overwrite `builds/<promoted-sha>/takt` (`:224-225`). story:self-improvement-protocol's D8 confines the validators' calls; this story confines the script itself, which reviewers run by hand today.

## Scope

- `scripts/check-finding`: the build step (cited)
- `crates/takt-cli/tests/finding_scripts.rs`: the confinement rows (inferred)

## Acceptance

A fixture revision whose `build.rs` writes `$HOME/canary` and copies its environment to a file inside the build directory leaves no `$HOME/canary`, and the copied environment holds no variable outside the allow-list; a build that writes into another revision's `builds/` directory fails.

## Landed (wave 13)

Read from `git diff --name-only 161b73f^1 161b73f` (the unit's merge into the wave branch) at the close of wave 13 (wave branch head 16ff976). check-finding builds the revision under check and runs the predicate in bubblewrap, the protocol sources bound read-only; a bwrap that did not run the command to its end gives no result. Shared merge with story:check-finding-positive-control. Wave 13 E1.

- `.agents/planner-agent.md`
- `.agents/reviewer-agent.md`
- `crates/takt-cli/tests/finding_scripts.rs`
- `crates/takt-cli/tests/finding_scripts_adv1.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
- `protocols/self-improvement/1.yaml`
- `scripts/check-finding`
- `scripts/file-finding`
