---
format: aep.planning-md/3
id: story:the-command-forge-merges-through-its-merge-entry
kind: story
status: active
title: The command forge reaches its configured merge entry
summary: repository.merge reads the target tip for the command forge
tags:
- priority:medium
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T17:52:15Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-27T17:52:20Z", actor: "human:timo", revision: 4}
---
# Story: the-command-forge-merges-through-its-merge-entry

## Outcome

A case on the command forge reaches `repository.merge`: the run file's required `merge` entry runs, because the merge target's tip the validator compares is answered for the command forge (through git on the tree or the run file's `read` entry, as the ESS says), not refused as unknown.

## Context

- The B2 adversary review (2026-10-27): `repository.merge` first asks the forge for the merge target's tip (`crates/takt/src/validator.rs:3805`); the command forge has no answer, so the step stops with an error and the required `merge` entry is never reached.
- Off the exit-0 path of the trials, which use `--forge bot`; deferred to after the release (D105).
- Red test: `adv_b2_the_required_merge_entry_is_reachable`, parked on branch `review/tr-b2-followup` in `crates/takt/tests/adv_b2_command_forge_followup.rs`.

## Acceptance

1. `adv_b2_command_forge_followup::adv_b2_the_required_merge_entry_is_reachable` passes: a granted case on the command forge runs the configured `merge` argv exactly once and records the merge.
2. The docs and ESS name how the target tip is read for the command forge.
