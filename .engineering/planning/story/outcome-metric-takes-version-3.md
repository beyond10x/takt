---
format: aep.planning-md/3
id: story:outcome-metric-takes-version-3
kind: story
status: implemented
title: The scorecard's outcome metric is at definition version 3, so a takt that knows version 2 refuses a newer scorecard
summary: Unperformable and DiskLow changed what the outcome ranks mean but the metric version stayed at 2
tags:
- finding
- rsi
refs:
- provider: finding
  reference: outcome-metric-takes-version-3
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T23:24:02Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T23:24:02Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T00:08:23Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: outcome-metric-takes-version-3

## Outcome

The scorecard's outcome metric is at definition version 3, so a takt that knows version 2 (0.2.3
and earlier) refuses a scorecard saved by a newer takt instead of judging it under ranks it does
not have, and a stored version 2 baseline is rebaselined before a version 3 candidate is compared
with it.

## Context

Found by the wave 15 close review at 0fae6305 (docs/waves/wave-15.md). Wave 15 changed what the
outcome metric means in two ways. `Unperformable` ranks with the budget stops
(`crates/takt/src/scorecard.rs:407` at 0fae6305), and a `--forge none` run that used to stop
`NoProgress` (rank 1) now ranks 2. A `DiskLow` trial takes no rank and denies its scenario a
verdict (`DISK_LOW`, `scorecard.rs:68`; `NOT_COMPLETED`, `scorecard.rs:416`). The
metric's version stayed 2 (`METRICS`, `scorecard.rs:79`), and `METRICS` says a changed definition
takes the next version.

That is what the version is for: `takt promote` refuses a scorecard with a metric definition it
does not know ("metric outcome is at definition version N; this takt knows version 2, and a
scorecard under another definition is not compared", `crates/takt/src/promotion.rs:1971`). With
the version unchanged, the review's probe has the installed 0.2.3 judge a scorecard of `DiskLow`
trials as worse (exit 3), where the tip refuses it (exit 1). The two judges read the same outcomes
differently, and the version number gives the older one no way to notice.

## Acceptance

1. `METRICS` gives `("outcome", 3)`; `takt scorecard` prints `outcome v3` and a saved scorecard
   records it: a test.
2. A takt built at 0fae6305 refuses, for its metric definition, a scorecard saved by the
   candidate: a test on a fixture saved under version 3, or a live check recorded in the review.
3. A stored version 2 baseline is compared with a version 3 candidate only after
   `takt promote --rebaseline`, as for any newer definition: a test.
4. At the candidate, `scripts/check-finding story:outcome-metric-takes-version-3 <candidate>`
   prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent "Remove a todo by its number" --harness none --forge none --test-cmd ./check.sh --min-free-gib 100000; "$TAKT" scorecard --repo .
exit: 0
healthy_exit: 0
output: outcome v2,
source: docs/waves/wave-15.md
```
