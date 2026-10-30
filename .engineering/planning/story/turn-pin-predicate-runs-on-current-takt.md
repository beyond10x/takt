---
format: aep.planning-md/3
id: story:turn-pin-predicate-runs-on-current-takt
kind: story
status: active
title: A judge-surface turn's finding predicate no longer runs on the current takt
summary: 'check-finding gives error at the candidate: the seed''s check-finding script, protocol file and cases path are all stale'
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: turn-pin-predicate-runs-on-current-takt
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:a-turn-pins-the-repository-of-its-forge
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T01:26:54Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T01:26:55Z", actor: "human:timo", revision: 3}
---
# A judge-surface turn's finding predicate no longer runs on the current takt

## What is wrong

`scripts/check-finding story:a-turn-pins-the-repository-of-its-forge <revision>` reports
`reproduced` at the promoted revision f4f8d8d2 and `error` at the candidate b0dd8047 (wave 40),
never `not_reproduced`. The predicate builds its repository from its seed 00a900ac, whose
`scripts/check-finding` hashes to a script the candidate no longer accepts as a reviewed confined
script (`crates/takt/src/improvement.rs`, the list changed in wave 35), so the candidate refuses
to start the turn and the predicate exits 2. A newer seed is not enough: the predicate names
`protocols/self-improvement/1.yaml`, removed when the protocol became the built-in
`self.improvement/1`, and writes evidence under `.engineering/cases/`, where cases no longer live.

No single seed satisfies both binaries: the promoted takt accepts only scripts the candidate
refuses, and the reverse.

## Check

```expected_before
command: bash scripts/check-finding story:a-turn-pins-the-repository-of-its-forge HEAD
exit: 0
healthy_exit: 0
output: error
source: .engineering/planning/story/a-turn-pins-the-repository-of-its-forge.md
```

## Outcome

story:a-turn-pins-the-repository-of-its-forge's predicate gives `not_reproduced` on the current
takt and `reproduced` on a revision before the fix, so its acceptance 4 can be verified.

## Acceptance

1. `check-finding story:a-turn-pins-the-repository-of-its-forge <candidate>` prints
   `not_reproduced`.
2. The same at a revision before bc4be287 prints `reproduced`.

## Scope

- `.engineering/planning/story/a-turn-pins-the-repository-of-its-forge.md` (cited)
