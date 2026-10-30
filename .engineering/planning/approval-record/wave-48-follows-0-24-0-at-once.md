---
format: aep.planning-md/3
id: approval-record:wave-48-follows-0-24-0-at-once
kind: approval-record
status: approved
title: Wave 48 opens as soon as 0.24.0 is released, and 0.25.0 follows its three units
summary: the Loom pin, the served Connectors instance and the git relay; units that need no release start first
relations:
- decides: story:git-and-model-through-loopback-relays
- decides: story:connectors-forge-speaks-github-at-an-endpoint
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T07:40:00Z", actor: "human:timo", revision: 2}
---
# Approval: wave 48 follows 0.24.0 at once

## Decision

The owner, 2026-10-30, through the orchestrator: once 0.24.0 is released and its image is
loaded, wave 48 opens without waiting for a further item. Its units:

1. the next Loom release's pin, when it is tagged, closing Acceptance 2 and 4 of
   story:git-and-model-through-loopback-relays;
2. the connectors forge against a real served Connectors instance (the release that serves the
   github operations) instead of the loopback stand-in;
3. push and fetch through the Connectors git relay (the release that ships it).

Units that need no release start first, against stand-ins, and each pin is swapped in as its
release lands. Release 0.25.0 follows when all three are in.

## Covered

| item | what |
|---|---|
| wave | 48, opened after 0.24.0's release and image |
| units | the Loom pin; the served Connectors instance; the git relay |
| release | 0.25.0 when all three are in |
