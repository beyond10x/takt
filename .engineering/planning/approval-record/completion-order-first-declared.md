---
format: aep.planning-md/3
id: approval-record:completion-order-first-declared
kind: approval-record
status: approved
title: A case completes on its first declared outcome, Loom's order
relations:
- decides: story:the-loop-is-run-until-blocked
- informed_by: dependency-blocker:loom-runtime-stops
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T00:00:27Z", actor: "human:timo", revision: 2}
---
## Decision

The coordinator, 2026-10-22, on gap E of dependency-blocker:loom-runtime-stops: when two outcomes of a case are legitimate at once, takt takes Loom's declaration order, the first declared outcome (`CompletionRule::FirstDeclared`, Loom 0.32.0 `crates/loom-governor/src/lib.rs:894`, `1334-1345`).

- Options were: A, takt takes Loom's declaration order; B, Loom adds a rule that completes on the first outcome by id; C, takt reorders its protocols' outcomes, a judge-surface change.
- A was taken: it needs no Loom change, and it matches the order each protocol declares its outcomes in.

## Consequence

- Today takt completes on the first legitimate outcome by id (`crates/takt/src/decision.rs:78-86`). With `accepted` and `abandoned` both legitimate, takt records `abandoned` today and `accepted` once story:the-loop-is-run-until-blocked lands; the exit code is 0 either way.
- Gap E is settled for that story. Gap C and the stops for active time and low disk stay open on the blocker.
