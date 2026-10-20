---
format: aep.planning-md/3
id: story:a-trial-pins-another-repositorys-candidate
kind: story
status: draft
title: A re-trial runs another repository's candidate revision pinned in place of the release, and a promotion records the full pin set
summary: 'ADR 0101: a proposal for another layer is re-trialled by pinning its candidate; promotion records every layer''s pin'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-trial-pins-another-repositorys-candidate

## Outcome

A re-trial can run with another repository's candidate revision (a harness session layer, the
model layer, the runtime, the tools) pinned in place of its released revision, and a promotion
record names the full set of pins the promoted binary was measured with.

## Context

ADR 0101 (2026-10-20): a proposal that concerns another repository is measured by pinning that
repository's candidate in the stack and re-running the trials; the candidate itself is delivered
in its own repository. Comes from self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/promotion.rs` (the record's pin set), `crates/takt/src/scorecard.rs`, the
  scenario run's build of the binary under test, `ess/domains/takt.yaml`, tests.

## Acceptance

1. A scenario run can be given a pin (`<layer>=<repository>@<revision>`) that replaces that
   layer's released revision in the binary under test; the run's record names it.
2. `takt promote` records the full pin set of the scorecard it promotes on, and refuses a
   scorecard whose runs do not share one pin set, naming the difference.
3. A promotion on a pin that is not a released revision of its repository is refused unless the
   owner's grant names it.
