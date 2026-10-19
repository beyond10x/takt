---
format: aep.planning-md/3
id: story:promotion-without-daily-cap
kind: story
status: active
title: takt promote allows more than one promotion a UTC day
summary: the same-day refusal removed; lock, recorded-already, no-regression and the chain stay
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: website/docs/self-improvement/scorecard.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T11:02:04Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T11:02:04Z", actor: "human:timo", revision: 4}
---
# Story: promotion-without-daily-cap

## Outcome

`takt promote` no longer refuses a promotion because another was made the same UTC day. Two
promotions of different revisions on one day both succeed when each scorecard shows no regression
against the baseline before it, and the second becomes the next baseline in the chain.

## Context

The owner removed the one-promotion-a-day default on 2026-10-19: improvement trials must run back to
back, and the cap held every trial until the next 00:00 UTC (the refusal "a promotion was made this
UTC day already: ... one promotion a day", `crates/takt/src/promotion.rs:2433` at wave 14's tip
8579d74). The cap came from the design's promotion defaults (design:recursive-self-improvement,
§ Scorecard) and story:promotion; a cap may return after 1.0. Everything else promotion checks
stays: one promotion at a time under the store's lock, a revision or tree recorded already is
refused, the no-regression rule, the chain of records and its head.

## Scope

- `crates/takt/src/promotion.rs`: the same-day refusal and its module doc (cited)
- `crates/takt-cli/src/main.rs`: the promote command's doc and its exit-1 list (cited)
- `crates/takt-cli/tests/promote_cli.rs`: the same-day tests, inverted (cited)
- `ess/domains/takt.yaml`: the promotion refusal list (cited)
- `website/docs/self-improvement/scorecard.md`: "One promotion a UTC day" (cited; doc-writer after merge)

## Acceptance

- A second `takt promote` of another revision on the same UTC day, with a scorecard showing no
  regression against the first, exits 0 and writes a record whose baseline is the first: a
  `promote_cli` test, red on 8579d74.
- A second `takt promote` of the same revision is still refused as recorded already.
- `takt promote --help` and `ess/domains/takt.yaml` name no daily cap; `task check` passes.
