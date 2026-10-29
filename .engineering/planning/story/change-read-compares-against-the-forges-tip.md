---
format: aep.planning-md/3
id: story:change-read-compares-against-the-forges-tip
kind: story
status: draft
title: change.read compares against the target as the forge reports it
summary: change.read reads the target's tip from the case tree's local branch, which a local-forge clone never updates, so its record names a stale tip while the merge reads the forge's
tags:
- finding
- priority:medium
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# change.read compares against the target as the forge reports it

## What is wrong

`change.read` takes the merge target's tip from the case tree's own `refs/heads/<target>`
(`crates/takt/src/validator.rs`, the `rev-parse ... refs/heads/{name}` above "its tip now"). On
the local forge a run works in a clone of the hub, and nothing updates that local branch after
the clone, so the comparison, its summary ("merging X into T at Y, its tip now") and the paths it
records name the target as it was when the clone was made. The merge itself reads the tip from
the forge (`Forge::target_tip`), so the record and the merge disagree about what the change was
compared with.

Seen in wave 44: U3's every `change.read` named `integration/wave-44 at f790c60f, its tip now`
while the hub's branch had moved to 2347c061 and then d9346d8c with two other units' merges.

## Check

```expected_before
command: grep -q 'refs/heads/{name}' crates/takt/src/validator.rs
exit: 0
healthy_exit: 1
source: crates/takt/src/validator.rs
```

## Outcome

`change.read` compares the case's revision with the target's tip as the case's forge reports it
(`Forge::target_tip`), as the merge does, and names that tip in its summary.

## Acceptance

1. The predicate above exits 1.
2. A local-forge case whose hub branch moved after the clone records a `change_record` that names
   the hub's tip, not the clone's.
