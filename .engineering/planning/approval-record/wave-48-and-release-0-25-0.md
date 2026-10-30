---
format: aep.planning-md/3
id: approval-record:wave-48-and-release-0-25-0
kind: approval-record
status: approved
title: Wave 48 and release 0.25.0
summary: wave 48 as proposed, b1 and c1 on the connectors 0.62.0 tag, and the 0.25.0 release
relations:
- decides: story:connectors-forge-at-a-served-instance
- decides: story:git-through-the-connectors-git-relay
- decides: story:git-and-model-through-loopback-relays
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T11:50:21Z", actor: "human:timo", revision: 2}
---
# Approval: wave 48 and release 0.25.0

## Decision

The orchestrator, under the owner's grant (2026-10-30), approved wave 48 as proposed in
docs/waves/wave-48.md, and the release that closes it.

1. Units a, b, c (committed), b1 and c1 (against connectors 0.62.0's served instance), the
   adversary and security-review passes on a, b and c, and fixes for their red findings.
2. b1 and c1 start as soon as connectors `v0.62.0` is on its origin; review passes do not hold
   a merge.
3. Unit commits are pushed to `wave/48` as they land, so a rehearsal image built from it carries
   them.
4. The release: tag 0.25.0 through takt's own process, verify the release, a three-line
   announcement, and the 0.25.0 image loaded into the local cluster.

## Covered

| item | what |
|---|---|
| commits | review-test and fix commits, b1, c1, their merges into `wave/48`, the closing store commit |
| delivery | pushes of `wave/48`, one PR through the bot, its merge into `main` |
| release | 0.25.0: tag, release workflow, verification, announcement, image |
