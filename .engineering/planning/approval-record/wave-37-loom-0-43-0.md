---
format: aep.planning-md/3
id: approval-record:wave-37-loom-0-43-0
kind: approval-record
status: approved
title: loom 0.43.0 and b10x-harness 0.20.0 join wave 37 (option A)
tags:
- loom-agent
relations:
- decides: story:image-harness-0-20-0
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T16:16:30Z", actor: "human:timo", revision: 2}
---
# Approval: loom 0.43.0 and b10x-harness 0.20.0 join wave 37

## Decision

The orchestrator, under the owner's grant (2026-10-26), a dependency upgrade it had agreed as
option A at the wave's open: "loom 0.43.0 is tagged (tag = loom main 92480ede, bot; metaharness
0.19.0, so harness 0.20.0 resolves through loom-sdk ...). If your wave's gate has not closed, pin
loom 0.43.0 and harness 0.20.0 in this wave as agreed (option A)". The gate had not closed: wave
37 was not merged.

## Covered

| item | what |
|---|---|
| U5 | story:image-harness-0-20-0: `b10x-loom-sdk` at loom 0.43.0 (metaharness 0.19.0, the harness crates 0.20.0) in both lockfiles, and the image's `b10x-harness` 0.20.0 (x86_64 SHA256 782da29650857aa1b6c9a118f05a64304fb434af775b46e9cabbb4f541964a80), one unit commit with an adversary pass |
| not covered | the environment credential stays `openai-responses` only: harness 0.20.0 still presents a token file as a subscription token on the Messages wire, and loom 0.43.0's session executor still has no API-key file route |
