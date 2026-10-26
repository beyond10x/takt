---
format: aep.planning-md/3
id: approval-record:wave-37-engineering-protocols-0-10-0
kind: approval-record
status: approved
title: engineering-protocols 0.10.0 joins wave 37; merged release heads are deleted
relations:
- decides: story:protocols-are-engineering-protocols-built-ins
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T13:59:34Z", actor: "human:timo", revision: 2}
---
# Approval: engineering-protocols 0.10.0 joins wave 37

## Decision

The orchestrator, under the owner's grant (2026-10-26), a dependency upgrade added to wave 37:
"engineering-protocols 0.10.0 is released (tag 0.10.0 ...; verify-release exit 0). Per Atlas ADR
0094 its three protocols story.delivery/1, self.improvement/1 and loop.improvement/1 are now
built-ins, byte-identical to takt 0.12.0 ... Unit: pin `b10x-canon-engineering` at tag 0.10.0;
drop `EMBEDDED` ... and `protocols/*/1.yaml`, keep `loop-improvement/metrics.yaml` ... Also in the
wave close: delete the remote `release/0.11.0` and `release/0.12.0` heads if their tips are on
main (bot route `-- delete-branch`). Record in your store; the release at the end covers it."

## Covered

| item | what |
|---|---|
| U6 | story:protocols-are-engineering-protocols-built-ins, one unit commit on `wave/37` with an adversary pass |
| branches | the remote heads `release/0.11.0` and `release/0.12.0`, deleted through the bot route only when each tip is an ancestor of `main` |

## Branches

Neither head was deleted: on 2026-10-26, before the merge, the tip of `release/0.11.0` (46619e4) and of `release/0.12.0` (f48584a) is not an ancestor of `main` (378159a); each release landed from its `-2` branch instead. Deleting an unmerged branch is outside the grant, so both stay.
