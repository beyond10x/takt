---
title: Roles
description: The owner and the four agent roles that work on takt, what each owns and what each never does.
---

# Roles

Four agent roles work on takt, beside the owner, a person. The split is the owner's decision of
2026-10-18. Each agent role starts from a profile in `.agents/<role>-agent.md`.

| role | owns | never |
|---|---|---|
| owner (a person) | merge approval for judge surfaces, or its delegation to a named role on record; scorecard definitions; priorities | |
| planner | the planning store, filing findings, deciding what the developer escalates, granting merges on a reviewer's verdict, dispatching the reviewer and the doc-writer, reporting to the owner | writes takt code; grants without a reviewer verdict on that exact revision |
| developer | waves and their pages, implementation, merges through the bot route, building and installing promoted binaries | marks its own fix verified; edits `docs/**` other than its wave pages |
| reviewer | a verdict with cited findings on each grant request at its exact revision; code reviews; adversarial passes; trials, scenario runs and verification runs; measurement against the baseline | writes takt code; grants; files to the store; reviews its own work |
| doc-writer | `docs/**` but the wave pages, the structure of `README.md`, this site | writes takt code; changes the planning store; states a figure no record holds |

## How work moves between them

The planning store carries the content. A message between agents only says that the store
changed, and its first line is fixed: `[FINDING takt] <ids>`, `[GRANT? takt] <story> <branch>
<sha> <paths>`, `[GRANT takt] <story> <sha>`, `[VERIFIED takt] <story> <scenario> <before> ->
<after>` and so on. The table is `AGENTS.md` § Messages.

A finding goes into the store with an `expected_before` predicate a machine can check. A fix is
verified by a run other than the one that delivered it.

## Grants on judge surfaces

A diff that touches a judge surface (every path but `docs/**`, `README.md` and `CHANGELOG.md`) is
merged under the owner's grant, or under a role's grant that the owner delegated on record. The
delegation is revocable. Until `story:judge-guard` lands, the record is the owner's decision in
the planning store, and takt does not enforce it.

Sources: `AGENTS.md` § Roles and § Messages, `design:recursive-self-improvement` § Roles.
