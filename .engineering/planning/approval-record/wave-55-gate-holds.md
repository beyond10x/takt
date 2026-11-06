---
format: aep.planning-md/3
id: approval-record:wave-55-gate-holds
kind: approval-record
status: approved
title: 'Wave 55: the gate holds against a step''s edits'
summary: one wave of four rsi gate stories for 0.33.0
relations:
- decides: story:the-gate-reads-paths-inside-arguments
- decides: story:the-gate-compares-taskfile-includes
- decides: story:the-gate-holds-against-lint-allows-in-source
- decides: story:the-gate-does-not-trust-a-step-accepted-citation
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T04:03:39Z", actor: "human:timo", revision: 2}
---
# Approval: wave 55

## Decision

The owner's standing delegation, through the orchestrator, 2026-11-06: "Go for wave 55 as proposed, release 0.33.0.
Accept the four rsi drafts: the-gate-reads-paths-inside-arguments, the-gate-compares-taskfile-includes,
the-gate-holds-against-lint-allows-in-source, the-gate-does-not-trust-a-step-accepted-citation."

takt has no planned wave left, and the one active story, story:harness-child-confined, is blocked
(dependency-blocker:loom-child-confinement). Wave 55 is drawn from four `rsi` drafts filed by the review of
story:the-gate-compares-its-scripts-and-tool-config-at-the-base. Each was read as still present on `main` at
9e7a3904e (`docs/waves/wave-55.md` § Selection).

The wave is four units, run one after the other in one tree in the order a, b, c, d, since all four change
`crates/takt/src/validator.rs`:

- unit a: story:the-gate-reads-paths-inside-arguments
- unit b: story:the-gate-compares-taskfile-includes
- unit c: story:the-gate-holds-against-lint-allows-in-source
- unit d: story:the-gate-does-not-trust-a-step-accepted-citation

All four are accepted by this record, not by the agents that filed them. An adversary pass and a security review run
on `wave/55` after unit d. The close-out of wave 54 (`store/close-wave-54`) is merged into `wave/55` first.

## Release

0.33.0: the gate refuses changes it accepted before (a path glued to a short option or inside `sh -c`, an included
Taskfile, a crate-level lint allow, a step-written citation digest or a dropped specification input).

## Commits this approval covers

The unit commits, the review fix commits, the release preparation, the store close, one pull request through the bot
route, its merge on green, the tag 0.33.0 and the release.
