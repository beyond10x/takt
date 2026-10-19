---
format: aep.planning-md/3
id: story:takt-commits-with-the-repositorys-identity
kind: story
status: active
title: takt commits with the identity the repository configures, never the operator's git config
summary: the first real turn's candidate carried takt@localhost and the owner's committer; bot rulesets now refuse that
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
# Story: takt-commits-with-the-repositorys-identity

## Outcome

Every commit takt makes (a take, `implementation.edit`, `tests.write`, a scenario run's trial
commits) carries the author and committer the repository configures for takt, never the operator's
git configuration; takt's own repository and its scenario repositories configure the bot identity,
so its commits pass the repositories' identity rulesets.

## Context

On 2026-10-19 the first real improvement turn's candidate reached takt's main with author
`takt <takt@localhost>` and the operator's own committer identity, read from the clone's git
configuration. The repository had no rulesets then; main was rewritten to the bot identity and
the bot rulesets (author and committer must be the bot) were added to takt and six other
repositories, the scenario repository included. Until takt commits with a configured identity, a
candidate takt delivers and every trial commit a scenario run pushes is refused.

## Acceptance

- With an identity configured for the repository (`.engineering/project.yaml` or a flag), every
  commit of a delivery and of a scenario run carries it as author and committer: a test.
- With none configured, takt refuses to commit rather than use the operator's git configuration,
  naming the setting.
- A scenario run against the scenario repository pushes through its identity ruleset.
