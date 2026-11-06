---
format: aep.planning-md/3
id: epic:work-sources
kind: epic
status: draft
title: 'Work sources: a run takes its work from the tracker a team plans in'
summary: '--work <key> and an ordered work list: aep, jira, github, gitlab, command'
relations:
- serves: vision:safe-agentic-coding
revision: 2
---
# Epic: work-sources

## Outcome

A run takes its work from the tracker a team plans in: takt's planning store (`aep`), a Jira instance, GitHub or
GitLab issues, or a team's own command. `--work <key>` names one item, its key naming its source; the run file's
ordered `work` list says where the next ready item comes from. `--forge` says where the change goes; `--work` says where
the work comes from.

## Shape (owner, 2026-11-06: "yeah, thats perfect")

- A source is named by the tracker, not by the transport: `aep`, `jira`, `github`, `gitlab`, `command`. Connectors is how
  takt reaches `jira`, `github` and `gitlab`; it is no source name. `command` serves a tracker no connector serves.
- A key names its source: `aep:story:x`, `jira:ABC-12`, `github:owner/repo#42`, `gitlab:group/project#7`; several
  connections of one tracker are told apart as `jira@<connection>:ABC-12`, the connection implicit when only one exists.
  `--story <id>` stays and means `--work aep:<id>`, with its ready check and its resume of an open case.
- The run file's `work` key (flat, as `forge_instance` is) lists sources in order, for example
  `work = ["aep", "jira@team-a?label=takt-ready"]`; takt takes the first ready item.
- The case keeps the item's key, its revision and its text as `intent.md` with its digest, as a story case does today
  (`crates/takt/src/case.rs`, the `intent` artifact revision); a later edit of the ticket changes nothing in an open case.
- The commit trailer `Refs:` (`crates/takt/src/commit_style.rs`) carries the item's key; an `aep` item keeps `Refs: <story id>`
  (approval-record:wave-56-work-model-points).
- The source runs in takt's control plane, outside the harness session: no tracker credential reaches the harness, a
  prompt or a case record.

## Stories

- Wave 56: story:work-items-in-the-ess-model, story:work-flag-and-the-aep-source, story:the-command-work-source.
- Wave 57: story:work-from-jira-github-and-gitlab, story:claim-and-report-on-a-tracker; first, in connectors-definitions,
  an issue-tracker profile and the Jira mapping.

## Out of scope

Importing tracker items into the planning store; a direct `jira`, `github` or `gitlab` client in takt beside
Connectors; scenarios, improvement turns and scorecards keyed by a tracker item (they stay on stories).
