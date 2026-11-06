---
format: aep.planning-md/3
id: approval-record:wave-56-work-sources
kind: approval-record
status: approved
title: 'Wave 56: work sources, the aep and command sources'
summary: one wave of four units for 0.34.0
relations:
- decides: story:work-items-in-the-ess-model
- decides: story:work-flag-and-the-aep-source
- decides: story:the-command-work-source
- decides: story:git-excluded-files-are-not-step-changes
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T13:11:50Z", actor: "human:timo", revision: 2}
---
# Approval: wave 56

## Decision

The orchestrator's go, 2026-11-06, under the owner's delegation, resting on the owner's "yeah, thats perfect"
(approval-record:work-sources-shape): "Go for wave 56 (takt 0.34.0) ... Accept at go:
story:work-items-in-the-ess-model, story:work-flag-and-the-aep-source, story:the-command-work-source, plus
story:git-excluded-files-are-not-step-changes as unit d. Units a-d serial in takt-w56-int as proposed; case-id scheme
accepted (aep unchanged; <source>-<slug>-<instant>, +8 hex sha256 when lossy, full key in the case). Adversary +
security reviewer after d. Wave 57 drafts stay draft until connectors-definitions has a Jira profile."

The wave is four units, run one after the other in one tree in the order a, b, c, d:

- unit a: story:work-items-in-the-ess-model, `ess/` only.
- unit b: story:work-flag-and-the-aep-source.
- unit c: story:the-command-work-source.
- unit d: story:git-excluded-files-are-not-step-changes (accepted by approval-record:wave-56-excluded-files).

The three work-source stories are accepted by this record, not by the agents that drafted them.
story:work-from-jira-github-and-gitlab and story:claim-and-report-on-a-tracker stay drafts.

## Release

0.34.0: `--work <key>` and the run file's `work` list are new; `--story` keeps its behaviour; a step no longer counts
the paths the run file names as the deployment's.

## Commits this approval covers

The unit commits, the review fix commits, the release preparation, the store close, one pull request through the bot
route, its merge on green, the tag 0.34.0 and the release.
