---
format: aep.planning-md/3
id: vision:safe-agentic-coding
kind: vision
status: in_review
title: Safe agentic coding, proven by composable agentic work
summary: Workflows of engineering protocols and loops, driven by a harness, validated by Canon over evidence, recorded in AEP; one binary pointed at a repository
revision: 2
transitions:
- {from: "draft", to: "in_review", at: "2026-10-18T02:34:09Z", actor: "human:timo", revision: 2}
---
# Vision: safe-agentic-coding

## Objective

Safe agentic coding, applied. A YAML description of workflows made of engineering protocols (a software change is one) and building blocks: inner loops, outer loops, validation loops. A harness such as Claude Code understands the protocols and the blocks and drives a workflow through the model, in any code repository, ad hoc or from the repository's default. Every step is validated by a hard gate: Canon evaluates the case over evidence, and AEP is the record.

The result is one binary. Point it at a repository and say "work on this": it pulls the next unit from the planning store, or takes an intent, opens a case, and runs one iteration, three, or three hours, by the stop criteria given. Everything is pluggable and composable.

## The two rules

Agents choose how to do the work; the organization defines what must be true before the work counts. A run completing or a pull request merging is not completion: completion is earned when the protocol's conditions hold.

## What serves it

Work serves this vision when it makes a unit of work finish under those rules with less of a person's time, keeps every claim on evidence a validator produced, or shows with numbers that bounded validated runs do work worth their cost. The design is `docs/design/takt.md`; epic:composable-agentic-work carries the first milestones.
