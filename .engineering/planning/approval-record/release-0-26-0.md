---
format: aep.planning-md/3
id: approval-record:release-0-26-0
kind: approval-record
status: approved
title: Release 0.26.0 of the twelve commits after 0.25.0
summary: 'the orchestrator''s grant under decision D322 to release what accumulated: scorecard refuses a missing repository, plus store records'
relations:
- decides: story:scorecard-refuses-missing-repo
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-31T08:16:18Z", actor: "human:timo", revision: 3, executor: "agent:takt-release"}
---
# Approval: release 0.26.0

## Decision

The orchestrator, under the owner's grant (2026-10-31, the owner's instruction to release what has accumulated), approved release 0.26.0 of the twelve commits on `main` after 0.25.0.

1. Version 0.26.0: the commits include the feature story:scorecard-refuses-missing-repo, and the repository's convention is a minor version for a feature at 0.x.
2. The release follows the repository's own process: a release pull request through the bot, merged when its checks are green, the tag on the merge, the release workflow, verification and an announcement.
3. Store-only commits since 0.25.0 ride the release pull request.

## Covered

| item | what |
|---|---|
| release pull request | version lines, CHANGELOG, the pages that name the latest release, this record |
| release | tag 0.26.0, the release workflow, verification, announcement |

## Not covered

A merge of a pull request that touches a judge surface still needs the owner's grant or a delegation on record; this record is not one.
