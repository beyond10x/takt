---
title: Glossary
description: The words takt's pages use, each in one or two plain sentences, with a link to the page that explains it.
lede: One or two sentences per word, and where to read more.
source: takt run --help, takt approve --help and takt protocols of takt 0.21.0; the concept pages this one links to; the READMEs of AEP, ESS and Substrate
---

# Glossary

## A run

### Case

One piece of work takt drives on one protocol. It is a directory, `.takt/cases/<case>/`, in your
repository: every decision, evidence record and event of the work. A later run resumes it.
[Protocol and case](./protocol-and-case.md#case).

### Protocol

A data file that says which claims must hold, which evidence makes them hold, which actions need
a grant and how the case can end. `takt protocols` lists the built-in ones; an intent opens on
`software.change/1`. [Protocol and case](./protocol-and-case.md#protocol).

### Story

A planned piece of work with its acceptance, kept in an AEP planning store. A run with no intent
takes the next ready story and delivers it on a story-delivery protocol.
[Run a story](../guides/run-a-story.md).

### Frontier

The governor's verdict on every action before a step: admissible, approval-required or blocked.
The console prints the admissible ones as `may:`; `--verbose` prints the whole frontier.
[The step loop and the frontier](./one-step-at-a-time.md#the-frontier).

### Harness

The coding agent that takes a judgement step: Claude Code (the default), Codex or b10x-harness.
It works in a scratch copy of your tree and proposes a change; takt applies it.
[A judgement step](./one-step-at-a-time.md#a-judgement-step).

### Governor

The part of Loom that decides each evaluation of the case: which actions are admissible and
which wait for a grant. It grants nothing itself.
[What takt is built on](./architecture.md#loom-sdk-runtime-governor-session-executor).

## Evidence and grants

### Validator

A program takt runs itself, never the model: the test command, the repository's gates, the
pull request, CI and the merge. Each writes evidence.
[Evidence and the judge](./evidence-and-the-judge.md#the-validators).

### Evidence

A `canon-evidence/1` record a validator writes, naming the commit it observed. Whether the work
is done is computed from these records, not from what the model says.
[Evidence and the judge](./evidence-and-the-judge.md#evidence-comes-from-validators).

### Grant

A recorded permission for one capability, such as `repository.merge`, given with `takt approve`
for the case as it stands. An action that needs one stops the run, exit 4, until it is given.
[Capabilities and approvals](./capabilities-and-approvals.md#recording-a-grant).

### Judge surface

A path whose change needs a person's grant to merge, or a role's under a delegation on record.
Every path is one unless `.engineering/judge-allow.yaml` at the merge target lists it.
[Evidence and the judge](./evidence-and-the-judge.md#the-judge-whose-grant-merges-a-judge-surface).

## Where it runs

### Forge

Where a change leaves your machine as a pull request: `gitlab`, `command` (the run file's
`forge_commands`), `connectors` (a Connectors instance), `local` (a bare repository on this
machine) or `none`. Without
`--forge`, takt picks one from `origin`. [Configuration](../reference/configuration.md#the-forge).

### Confinement

How the test command is started. `none`, the default, runs it on the host as you, unconfined;
`substrate` runs it with no network and read-only toolchains, or refuses by name.
[Install](../guides/install.md#confinement).

### Substrate

The beyond10x library takt starts the test command through. Its adapters (`local`, `linux`,
`systemd`, `macos`) are what `--confinement` picks from.
[Substrate on GitHub](https://github.com/beyond10x/substrate).

## The projects around it

### Canon

The engine that evaluates a protocol over a case's evidence: claims, obligations, admissible
actions and outcomes. [Canon](https://beyond10x.github.io/canon/).

### Loom

The runtime takt is built on. It steps the case, its governor decides, and its session executor
runs each harness session. [What takt is built on](./architecture.md).

### AEP

Agentic Engineering Protocol: a repository's plan as Markdown files under `.engineering/planning/`,
read and changed with the `aep` command. takt takes ready stories from it.
[AEP](https://beyond10x.github.io/ecosystem/aep/).

### ESS

Executable System Specification: systems described as typed data and checked against what was
declared. takt's own specification is `ess/` in its repository; you do not need ESS to run takt.
[ESS](https://beyond10x.github.io/ess/).
