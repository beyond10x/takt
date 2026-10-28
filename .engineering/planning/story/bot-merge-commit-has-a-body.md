---
format: aep.planning-md/3
id: story:bot-merge-commit-has-a-body
kind: story
status: implemented
title: takt's merge through the bot forge has a title and a body naming the story and its evidence
summary: no default forge merge message
tags:
- priority:high
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T13:11:31Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-28T13:11:32Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-28T15:54:59Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

The merge commit takt makes through the bot forge has a title and a body: the title names the pull request and the story, the body the story, the merged revision and the evidence it merged on, instead of the forge's default message.

## Context

A delivery by takt 0.17.0 on beyond10x/tools (2026-10-28) merged its pull request with GitHub's default merge commit, `Merge pull request #18 from ...`, which has no body; commits in these repositories carry a body.

## Scope

- `crates/takt/src/forge/bot.rs`: `commit_title` and `commit_message` of the merge request
- `crates/takt/tests/`

## Acceptance

- `bot_merge_carries_a_body`: the bot forge's merge request holds a `commit_title` naming the pull request and the story and a `commit_message` naming the story, the merged revision and the evidence records.
