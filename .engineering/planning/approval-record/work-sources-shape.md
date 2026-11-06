---
format: aep.planning-md/3
id: approval-record:work-sources-shape
kind: approval-record
status: approved
title: The shape of work sources
summary: 'owner, 2026-11-06: yeah, thats perfect'
relations:
- decides: epic:work-sources
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T13:10:33Z", actor: "human:timo", revision: 2}
---
# Decision: the shape of work sources

## Decision

The owner, 2026-11-06, relayed by the orchestrator, on the shape in epic:work-sources § Shape: "yeah, thats perfect".

The owner's earlier words that set the naming: "Work minus minus work connectors. What should that mean? I mean, I could
be connected to Jira, GitLab, all of them." So a source is named by the tracker (`aep`, `jira`, `github`, `gitlab`,
`command`), a key names its source (`jira@<connection>:ABC-12` where several connections of one tracker exist), and the
run file's `work` key lists sources in order.

The split the owner approved: wave 56 is the ESS model, `--work` with the `aep` source, the `command` source, and
story:git-excluded-files-are-not-step-changes (accepted for wave 56 by approval-record:wave-56-excluded-files); wave 57
is `jira`, `github` and `gitlab` through Connectors, after an issue-tracker profile and the Jira mapping land in
connectors-definitions.

This record settles the shape. The stories of wave 56 are accepted at the wave's go, not by this record.
