---
title: Roles
description: The owner and the four agent roles that work on takt, what each owns and what each never does.
---

# Roles

Four agent roles work on takt, beside the owner, a person. Each agent role starts from a profile
in `.agents/<role>-agent.md`.

| role | owns | never |
|---|---|---|
| owner (a person) | merge approval for judge surfaces, or its delegation to a named role on record; scorecard definitions; priorities | |
| planner | the planning store, filing findings, deciding what the developer escalates, granting merges on a reviewer's verdict, dispatching the reviewer and the doc-writer, reporting to the owner | writes takt code; grants without a reviewer verdict on that exact revision |
| developer | waves and their pages, implementation, merges through the bot route, building and installing promoted binaries | marks its own fix verified; edits `docs/**` other than its wave pages, this site or the deck |
| reviewer | a verdict with cited findings on each grant request at its exact revision; code reviews; adversarial passes; trials, scenario runs and verification runs; measurement against the baseline | writes takt code; grants; files to the store; reviews its own work |
| doc-writer | `docs/**` but the wave pages, the structure of `README.md`, this site | writes takt code; changes the planning store; states a figure no record holds |

## How work moves between them

The planning store holds the content. A message between agents only announces a store change,
with a fixed first line
([`AGENTS.md` § Messages](https://github.com/beyond10x/takt/blob/main/AGENTS.md#messages)). A
finding goes into the store with an `expected_before` predicate a machine can check.

## Grants on judge surfaces

Which paths are judge surfaces, who grants their merge, and what takt enforces of it:
[Evidence and the judge](./evidence-and-the-judge.md#the-judge-for-takts-own-changes).

Sources: `AGENTS.md` § Roles and § Messages, `design:recursive-self-improvement` § Roles.
