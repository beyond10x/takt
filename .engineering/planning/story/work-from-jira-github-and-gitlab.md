---
format: aep.planning-md/3
id: story:work-from-jira-github-and-gitlab
kind: story
status: draft
title: The jira, github and gitlab sources read and select items through Connectors
summary: one key or the next ready item, no credential in takt
relations:
- decomposes: epic:work-sources
- serves: vision:safe-agentic-coding
- depends_on: story:work-flag-and-the-aep-source
revision: 1
---
# Story: work-from-jira-github-and-gitlab

## Outcome

The `jira`, `github` and `gitlab` sources read one item by its key and select the next ready one (by a label or a
query in the `work` entry) through a Connectors instance, with no credential in takt, the harness or the case. A
tracker with several connections is named as `jira@<connection>`.

## Context

The connectors forge reaches a Connectors instance by the `connectors` CLI or `forge_endpoint`
(`crates/takt/src/forge/connectors.rs`) and serves only `gitlab` and `github` today. A Jira source needs an
issue-tracker operation profile and a Jira mapping in connectors-definitions first, delivered there.

## Acceptance

- Against a fake Connectors endpoint: `--work jira:ABC-12` opens a case holding the item's text and key; a label
  selection takes the first ready item; a test each.
- `jira@team-a:ABC-12` picks that connection; a bare `jira:` key with two connections is refused, naming them.
- No credential reaches a child takt starts for the source: a test.
