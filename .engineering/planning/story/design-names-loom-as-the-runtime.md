---
format: aep.planning-md/3
id: story:design-names-loom-as-the-runtime
kind: story
status: implemented
title: The design, README and changelog name loom-sdk as the runtime takt runs on
summary: docs/design/takt.md, README.md and CHANGELOG.md describe takt on loom-sdk; not a judge surface
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:takt-evaluation-code-retired
- depends_on: story:second-harness-adapter
- depends_on: story:confinement-seam
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T16:10:52Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-22T16:10:52Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-23T02:11:33Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: design-names-loom-as-the-runtime

## Outcome

A reader of `docs/design/takt.md`, `README.md` and `CHANGELOG.md` learns that takt is an application on loom-sdk: the Commission runtime drives the case, the governor of Loom decides it with Canon, the harnesses are session executors that propose over metaharness, and takt keeps validators and evidence, git and forge, the planning-store seam, scenarios, scorecard, promotion, trace, report and view.

## Context

- The design says today that the loop is in this binary and rules out any runtime of its own for model turns (`docs/design/takt.md:80-81`); § 3 Shape draws the loop takt runs itself (`docs/design/takt.md:32-40`).
- Not a judge surface (`AGENTS.md` § Rules): every path in scope is under `docs/**`, `README.md` or `CHANGELOG.md`. `docs/**` and the structure of `README.md` are the doc-writer's (`AGENTS.md` § Roles).
- The architecture decision is ADR 0090 (Loom), with the owner's decision of 2026-10-20 that Loom reaches agent harnesses only through metaharness; ADRs are cited by number as text.
- The session-executor crate is built in this repository first and is meant to move into Loom unchanged; the design says so and says what stays in takt.

## Scope

- `docs/design/takt.md` (§ 2 Foundations, § 3 Shape, § 4 Milestones, § 5 Not in this design)
- `README.md`
- `CHANGELOG.md` (`## [Unreleased]`)

## Acceptance

1. `docs/design/takt.md` § 3 names, for each part of the loop, what performs it now — the Commission runtime, the governor, a session executor, an effect, a validator — with the crate or path that holds it.
2. No sentence of `docs/design/takt.md` says the loop is in this binary or excludes a runtime that takt runs on; § 5 says what is still excluded.
3. `README.md` names loom-sdk and the tag takt pins, and the session-executor crate.
4. `CHANGELOG.md` `## [Unreleased]` has an entry for each story of `epic:takt-on-loom-sdk` that has landed, each naming what a user of `takt run` sees change.
5. Every repository path the changed text names exists at the tree it is merged on; the diff touches no path outside `docs/**`, `README.md` and `CHANGELOG.md`.

## Out of Scope

The public site under `website/`; `AGENTS.md` (`story:takt-evaluation-code-retired`, `story:governor-holds-takts-cases`).
