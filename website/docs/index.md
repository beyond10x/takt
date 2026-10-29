---
title: What takt is
slug: /
sidebar_label: Overview
sidebar_position: 1
description: takt lets a coding agent such as Claude Code change your repository one checked step at a time. A program runs your tests after each step, a merge waits for your grant, and every run leaves a record.
---

# What takt is

takt lets a coding agent change your git repository one step at a time, and checks each step
with a program, not with the model. You give it an intent, such as "add a function sub", or a
story from your plan. Claude Code makes the change. takt commits it, runs your tests itself and
records what they said about that exact commit as [evidence](./concepts/glossary.md#evidence). A
merge waits for your [grant](./concepts/glossary.md#grant). Every run ends on a
named stop with its own exit code, so a script can drive it.

**Who it is for:** a developer who already lets a coding agent work on a repository, has a test
command, and wants "done" to mean that the tests passed at that commit and that a person granted
the merge.

**What it needs:** Linux, or macOS on Apple silicon; `git`; your test command (`cargo test
--quiet` by default); and Claude Code at a qualified version
([what it needs](./getting-started.md#what-it-needs)). It is one open-source binary, Apache-2.0.

## One command

In a clean, committed repository:

```sh
takt run --repo . --forge none --intent "add a function sub(left, right) that subtracts right from left, with a test"
```

Some lines of what it printed in [Getting started](./getting-started.md#2-a-run-with-claude-code):

```console
step 1: tests.run {} · pass · 0.3s
step 2: implementation.edit (claude-code) · 22.0s · 37.9k tok (27.7k cached) · $0.11
  effect: committed 1 path(s); implementation is now c3cda2f5: src/lib.rs
step 3: tests.run {} · pass · 0.6s
step 4: repository.merge (claude-code) · 11.7s · 14.5k tok (11.4k cached) · $0.04
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 4, tokens 52345, wall 41.7s, cost $0.15, commits 1 (1 path), case ~/takt-demo/.takt/cases/takt-20261029T110612Z
```

Claude Code edited; takt ran the tests before and after; the merge waits for `takt approve`.
`--forge none` names no [forge](./concepts/glossary.md#forge): the change stays on your machine, as a
commit on the branch `takt/<case>`.

Two words carry the rest of these pages:

- A **[case](./concepts/glossary.md#case)** is one piece of work, kept as files in
  `.takt/cases/<case>/`: every decision, every test result and every event. A later run resumes
  it.
- A **[story](./concepts/glossary.md#story)** is a planned piece of work with its acceptance,
  kept in an AEP planning store. With no intent, takt takes the next ready one.

The rest are in the [Glossary](./concepts/glossary.md).

## Where it sits

takt is not the coding agent, the planner or your CI: Claude Code, Codex or b10x-harness make the
edits, AEP holds the plan, and your forge runs the checks. How it fits with Canon, Loom and the
rest: [What takt is built on](/docs/concepts/architecture).

## What it costs

On scenario 1, a 31-line change to a toy command-line program and the only scenario with
free-run arms, takt is slower and dearer than Claude Code running free with takt's isolation and
a shell: about 1.8x the cost and 2.4x the wall time ([Runs](/docs/self-improvement/runs)). In
return it records evidence, grants, a case record and named stops. One toy scenario and small
samples: these are first results, not a benchmark.

## Next

- [Getting started](./getting-started.md): install takt 0.21.0, run it with no model, then with
  Claude Code, and take the change.
- [Run a story](./guides/run-a-story.md): deliver a story from a planning store.
- [Known limitations](./known-limitations.md): what does not work yet.
