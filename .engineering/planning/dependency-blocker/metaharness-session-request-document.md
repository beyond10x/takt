---
format: aep.planning-md/3
id: dependency-blocker:metaharness-session-request-document
kind: dependency-blocker
status: open
title: metaharness publishes no session-request document with unknown keys refused and a per-kind check
relations:
- blocks: story:harness-section-is-metaharness-session-request
- blocks: story:harness-flags-spell-metaharness-names
revision: 2
---
# Dependency blocker: metaharness-session-request-document

## What blocks

story:harness-section-is-metaharness-session-request (acceptance items 1, 4 and 5) and
story:harness-flags-spell-metaharness-names (acceptance items 1 and 3) need metaharness to publish its session request
as a document: `RunSpec` with unknown keys refused, a public parse, and `check_spec` over it, so that a bad value and a
key the harness kind does not take are refused in metaharness's own words
(epic:run-config-proxies-llm-and-metaharness § Out of Scope).

metaharness's store at `origin/main` 75a44bcc (tag 0.22.0) holds no story for it: no story under
`.engineering/planning/story/` names `RunSpec` or a session-request document (`git grep -il`, 2026-10-28).

## What clears it

A metaharness release whose public API parses a session request with unknown keys refused and checks it per harness
kind, reached by takt through loom-sdk. takt's tests `a_flag_not_carried_yet_is_refused_before_any_case_opens` and
`a_key_not_carried_yet_is_refused_not_dropped` then change from refusals to carried keys.
