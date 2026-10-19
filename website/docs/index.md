---
title: What takt is
slug: /
sidebar_label: Overview
description: One open-source binary that drives a repository's work through engineering protocols, every step checked by a validator.
---

# What takt is

takt is one open-source binary, Apache-2.0, that drives a repository's work through engineering
protocols. Point it at a repository and say "work on this". It takes an intent, or the next ready
story from the planning store, opens a case on a protocol, and lets a harness such as Claude Code
take one admissible step at a time. Each step's result is checked by a program, not by the model
that made it. [Canon](https://github.com/beyond10x/canon) decides what is permitted, what is owed
and whether completion was earned. The model never does.

> Agents choose how to do the work. The protocol defines what must be true before the work counts.

## The problem it answers

A coding agent running free decides when it is done and says so in text. Nothing ties that claim
to the commit that gets merged, and the same agent usually holds the rights to push and merge.

| question | a harness running free | takt |
|---|---|---|
| When is it done? | the agent decides and says so in text | computed from validator records |
| Did the tests pass on the merged commit? | nothing binds the two | evidence is bound to a revision |
| Who may merge? | the agent holds push and merge rights | only under a recorded grant |
| What was promised, and what was observed? | not recorded | a case record in the repository |
| When does it stop? | ad hoc | budgets, named stops, exit codes |

## What it costs

On the one scenario measured so far, takt is slower and dearer than Claude Code running free
with takt's isolation and a shell: about 1.8x the cost and 2.4x the wall time
([Runs](./self-improvement/runs.mdx)). takt's case is what the free runs do not give: evidence,
grants, a record and named stops.

## Where to go next

- [Getting started](./getting-started.md): install the 0.1.0 binary and run one step.
- [Protocol and case](./concepts/protocol-and-case.md): the two nouns everything else rests on.
- [Status](./status.mdx): every implemented story, and what is in the open wave.
- [The protocol viewer](pathname:///takt/protocols/): takt's protocols, drawn from the protocol
  files themselves.
