---
format: aep.planning-md/3
id: story:a-delivery-hands-its-candidate-to-a-turn
kind: story
status: active
title: A delivery that ends green hands a turn its candidate with no operator step
summary: the first real turn's candidate was cherry-picked by the operator
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T18:37:46Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T18:37:46Z", actor: "human:timo", revision: 3}
---
# Story: a-delivery-hands-its-candidate-to-a-turn

## Outcome

A delivery that ends after a green test step yields a candidate revision a self-improvement turn can
take as it is: the code commits on the mainline's tip, without the take commit, named by takt, with
no operator step.

## Context

The first real improvement turn (2026-10-19) took a candidate the operator assembled: the delivery
stopped NoProgress after its test step, and its two code commits were cherry-picked onto main's tip
without the take commit, which changes the finding's story file and would disqualify the candidate.
The turn's records hold, but the candidate was not handed over by takt.

## Acceptance

- A delivery under `--forge none` that reaches a green test step ends with a named stop that prints
  a candidate revision: the code commits replayed on the base, the take commit left out.
- A self.improvement/1 turn opened on that revision passes its opening checks: a test, end to end.
