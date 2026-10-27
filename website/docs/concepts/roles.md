---
title: Roles
sidebar_position: 7
description: The owner and the agent roles that work on takt itself, what each owns and what each never does.
lede: takt is built by an owner, a person, four agent roles and a trial agent; the planner grants on a reviewer's verdict, the developer never verifies its own fix, the reviewer never grants, and the trial agent measures and promotes but never merges its own candidate.
source: AGENTS.md § Roles and § Messages; .agents/*-agent.md
---

# Roles

These are the roles that work on takt's own repository, beside the owner, a person. Four agent
roles each start from a profile in `.agents/<role>-agent.md`; the trial agent runs the trials
beside them.

| role | owns | never |
|---|---|---|
| owner (a person) | merge approval for judge surfaces, or its delegation to a named role on record; scorecard definitions; priorities | |
| planner | the planning store, filing findings, deciding what the developer escalates, granting merges on a reviewer's verdict, dispatching the reviewer and the doc-writer, reporting to the owner | writes takt code; grants without a reviewer verdict on that exact revision |
| developer | moving stories to active, waves and their pages, implementation, merges through the bot route | marks its own fix verified; edits `docs/**` other than its wave pages, or this site |
| reviewer | a verdict with cited findings on each grant request at its exact revision; code reviews; adversarial passes; verification runs of a finding on request | writes takt code; grants; files to the store; reviews its own work |
| doc-writer | `docs/**` but the wave pages, the structure of `README.md`, this site | writes takt code; changes the planning store; states a figure no record holds |
| trial agent | trials and scenario runs, scorecards and measurement against the baseline, proposals and candidates, the promotion of a merged candidate and the install of its binary (`takt promote`) | edits a wave's tree or branch or `main`; merges its own candidate; reviews its own proposal; files to the store |

Until takt drives this flow itself, one session, the dispatcher, holds the planner's and the
developer's roles in one checkout. The trial agent is the one other session.

## How work moves between them

The planning store holds the content. Inside the dispatcher's session a sub-agent's return is the
message. Between the dispatcher and the trial agent there are four hand-overs, each a fixed first
line: a release to try, an accepted candidate, a rejected candidate, and a merged candidate to
promote ([`AGENTS.md` § Messages](https://github.com/beyond10x/takt/blob/main/AGENTS.md#messages)).
A finding goes into the store with an `expected_before` predicate a machine can check.

## Grants on judge surfaces

Which paths are judge surfaces, who grants their merge, and what takt enforces of it:
[Evidence and the judge](./evidence-and-the-judge.md#the-judge-whose-grant-merges-a-judge-surface).
