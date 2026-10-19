---
format: aep.planning-md/3
id: story:promotion-chain-head-anchored
kind: story
status: draft
title: Removing the latest promotion record is detected
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
## Outcome

`takt promote` detects a promotion record chain whose latest record was removed or replaced: the chain's head (the latest record's name and sha256) is kept outside the record directory, where a candidate binary's runs do not write, and a chain that ends before that head is refused, naming the missing record.

## Context

From the grant review of the wave-13 promotion stories (E2, 2026-10-19), finding W4: nothing anchors the last record of the chain (`crates/takt/src/promotion.rs:334-440`); deleting the latest `<rev>.refused.json` and promoting again without `--rejudge` exits 0, so a refusal can be undone by deleting one file. Already true before wave 13. Every record but the last is bound by the next one's `previous_sha256`.

## Scope

- `crates/takt/src/promotion.rs`: the record chain's read and write (cited)

## Acceptance

After a refusal, deleting the latest record and running `takt promote` on the same revision without `--rejudge` exits 1 naming the missing record; replacing it with an older record's bytes is refused the same way.
